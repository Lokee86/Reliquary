use super::{
    LegacyManagedSessionState, ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation,
};
use crate::{InteractionCompletion, InteractionReceipt, InteractionRole, InteractionSession};
use std::path::PathBuf;
use uuid::Uuid;

impl ReliquaryRuntimeHost {
    fn managed_session_state(
        &self,
        owner_id: &str,
    ) -> Result<LegacyManagedSessionState, ReliquaryRuntimeHostError> {
        self.execution_for(owner_id)?;
        Ok(self
            .managed_sessions
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
            .cloned()
            .unwrap_or_default())
    }

    fn update_managed_session_state(
        &self,
        owner_id: &str,
        f: impl FnOnce(&mut LegacyManagedSessionState),
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.execution_for(owner_id)?;
        let mut sessions = self
            .managed_sessions
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        f(sessions.entry(owner_id.to_owned()).or_default());
        Ok(())
    }

    pub fn active_session_id(&self) -> Result<Option<String>, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        self.active_session_id_for(&owner_id)
    }

    pub fn active_session_id_for(
        &self,
        owner_id: &str,
    ) -> Result<Option<String>, ReliquaryRuntimeHostError> {
        Ok(self.managed_session_state(owner_id)?.active_session_id)
    }

    pub fn require_active_session(
        &self,
        session_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let active = self
            .active_session_id()?
            .ok_or_else(|| operation("no conversation is active"))?;
        if active == session_id {
            Ok(())
        } else {
            Err(operation("conversation is not the active session"))
        }
    }

    pub fn start_conversation_session(
        &self,
    ) -> Result<InteractionSession, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        self.close_managed_session_for(&owner_id)?;
        let session_id = Uuid::new_v4().to_string();
        let session = self.with_runtime_for(&owner_id, |runtime| {
            runtime
                .open_session(session_id.clone(), None)
                .map_err(operation)
        })?;
        self.update_managed_session_state(&owner_id, |state| {
            state.active_session_id = Some(session_id);
            state.reopened_session_pending = false;
        })?;
        Ok(session)
    }

    pub fn reopen_conversation_session(
        &self,
        session_id: String,
        leaf_message_id: String,
    ) -> Result<InteractionSession, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        self.close_managed_session_for(&owner_id)?;
        let session = self.with_runtime_for(&owner_id, |runtime| {
            let session = runtime
                .open_session(session_id.clone(), Some(leaf_message_id))
                .map_err(operation)?;
            let changed = runtime
                .cva
                .set_conversation_active(&session_id, true)
                .map_err(operation)?;
            if changed {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(session)
        })?;
        self.update_managed_session_state(&owner_id, |state| {
            state.active_session_id = Some(session_id);
            state.reopened_session_pending = true;
        })?;
        Ok(session)
    }

    pub fn reopened_session_pending(
        &self,
        session_id: &str,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        Ok(self
            .managed_session_state(&owner_id)?
            .reopened_session_pending)
    }

    pub fn mark_reopened_session_resumed(
        &self,
        session_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        self.update_managed_session_state(&owner_id, |state| {
            state.reopened_session_pending = false;
        })
    }

    pub fn close_active_session(
        &self,
    ) -> Result<Option<InteractionSession>, ReliquaryRuntimeHostError> {
        let owner_id = self
            .active_rel_id()
            .ok_or_else(|| operation("Reliquary runtime has no active REL"))?;
        self.close_managed_session_for(&owner_id)
    }

    pub(super) fn close_managed_session_for(
        &self,
        owner_id: &str,
    ) -> Result<Option<InteractionSession>, ReliquaryRuntimeHostError> {
        let session_id = self.managed_session_state(owner_id)?.active_session_id;
        let Some(session_id) = session_id else {
            return Ok(None);
        };
        let session = self.close_session_for(owner_id, &session_id)?;
        if session.leaf_message_id.is_some() {
            self.with_runtime_for(owner_id, |runtime| {
                let changed = runtime
                    .cva
                    .set_conversation_active(&session_id, false)
                    .map_err(operation)?;
                if changed {
                    runtime.cva.sync().map_err(operation)?;
                }
                Ok(())
            })?;
        }
        self.update_managed_session_state(owner_id, |state| {
            state.active_session_id = None;
            state.reopened_session_pending = false;
        })?;
        Ok(Some(session))
    }

    pub fn persist_active_message(
        &self,
        session_id: &str,
        content: String,
        role: InteractionRole,
        project_dir: Option<PathBuf>,
        attachment_paths: Vec<PathBuf>,
    ) -> Result<InteractionReceipt, ReliquaryRuntimeHostError> {
        if content.trim().is_empty() && attachment_paths.is_empty() {
            return Err(operation("message content or attachment is required"));
        }
        self.require_active_session(session_id)?;
        let message_id = Uuid::new_v4().to_string();
        let timestamp_ns = crate::insomnia::runtime_step::now_ns();
        self.begin_message(session_id, message_id.clone(), role, timestamp_ns)?;
        if let Err(error) = self.append_text(session_id, &message_id, &content) {
            let _ = self.cancel_message(session_id, &message_id);
            return Err(error);
        }
        if !attachment_paths.is_empty() {
            let project_dir = project_dir
                .as_deref()
                .ok_or_else(|| operation("file attachments require a repository-associated REL"))?;
            for path in attachment_paths {
                let file = match self.ingest_project_attachment(project_dir, &path) {
                    Ok(file) => file,
                    Err(error) => {
                        let _ = self.cancel_message(session_id, &message_id);
                        return Err(error);
                    }
                };
                if let Err(error) = self.attach_project_file(session_id, &message_id, file) {
                    let _ = self.cancel_message(session_id, &message_id);
                    return Err(error);
                }
            }
        }
        let receipt = self.complete_message(session_id, &message_id)?;
        self.set_conversation_active(session_id, true)?;
        Ok(receipt)
    }

    pub fn begin_active_assistant_stream(
        &self,
        session_id: &str,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        let message_id = Uuid::new_v4().to_string();
        self.begin_message(
            session_id,
            message_id.clone(),
            InteractionRole::Agent,
            crate::insomnia::runtime_step::now_ns(),
        )?;
        Ok(message_id)
    }

    pub fn checkpoint_active_assistant_stream(
        &self,
        session_id: &str,
        message_id: &str,
        delta: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        self.append_checkpointed_text(session_id, message_id, delta)
    }

    pub fn interrupt_active_assistant_stream(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        self.interrupt_message(session_id, message_id)
    }

    pub fn complete_active_assistant_stream(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<InteractionCompletion, ReliquaryRuntimeHostError> {
        self.require_active_session(session_id)?;
        let policy = self.active_execution()?.episode_policy;
        self.complete_live_message(
            session_id,
            message_id,
            policy,
            crate::insomnia::runtime_step::now_ns(),
        )
    }
}
