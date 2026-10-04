use super::gateway::mount::MountClaim;
use super::{
    OwnerExecution, ReliquaryRuntimeHost, ReliquaryRuntimeHostError, notify_work, operation,
    owner_key, perception_flow, perception_owner, perception_queue,
};
use crate::InteractionRuntime;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl ReliquaryRuntimeHost {
    pub fn mount_rel(
        &mut self,
        mut runtime: InteractionRuntime,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let owner_id = runtime.cva().owner_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("mounted REL requires a durable owner ID".into())
        })?;
        let key = owner_key(Some(&owner_id));
        if self.executions.contains_key(&key) {
            return Err(ReliquaryRuntimeHostError::Operation(format!(
                "REL owner {owner_id} is already mounted"
            )));
        }
        let claim = MountClaim::acquire_legacy_rel(&runtime.cva().container)?;
        let metadata = runtime.cva().rel_metadata();
        self.validate_candidate_topology(&owner_id, &metadata.dependencies)?;
        let active_ids = runtime
            .conversation_summaries()
            .into_iter()
            .filter(|summary| summary.active)
            .map(|summary| summary.conversation_id)
            .collect::<Vec<_>>();
        let mut cleared = false;
        for conversation_id in active_ids {
            cleared |= runtime
                .cva
                .set_conversation_active(&conversation_id, false)
                .map_err(operation)?;
        }
        if cleared {
            runtime.cva.sync().map_err(operation)?;
        }

        let execution = OwnerExecution::start(
            runtime,
            Arc::clone(&self.routes),
            Arc::clone(&self.phylactery),
            self.config.clone(),
            self.episode_policy,
            false,
            false,
        );
        self.executions.insert(key.clone(), execution);
        self.legacy_rel_claims.insert(key, claim);
        Ok(owner_id)
    }

    pub fn unmount_rel(&mut self, owner_id: &str) -> Result<crate::Cva, ReliquaryRuntimeHostError> {
        self.close_managed_session_for(owner_id)?;
        self.managed_sessions
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .remove(owner_id);
        let key = owner_key(Some(owner_id));
        if self.active_key.as_deref() == Some(key.as_str()) {
            self.active_key = None;
        }
        let claim = self.legacy_rel_claims.remove(&key).ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("REL execution claim is unavailable".into())
        })?;
        let execution = self.executions.remove(&key).ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation(format!("REL owner {owner_id} is not mounted"))
        })?;
        execution.insomnia_enabled.store(false, Ordering::SeqCst);
        execution.phylactery_enabled.store(false, Ordering::SeqCst);
        let cva = execution.into_cva()?;
        drop(claim);
        Ok(cva)
    }

    pub fn set_active_rel(&mut self, owner_id: &str) -> Result<bool, ReliquaryRuntimeHostError> {
        let key = owner_key(Some(owner_id));
        if !self.executions.contains_key(&key) {
            return Err(ReliquaryRuntimeHostError::Operation(format!(
                "REL owner {owner_id} is not mounted"
            )));
        }
        self.dependency_closure(owner_id)?;
        if self.active_key.as_deref() == Some(key.as_str()) {
            return Ok(false);
        }

        let (principal_profile, user_keys) = self.activation_phy_state()?;
        if let Some((principal_id, profile)) = principal_profile {
            let runtime = self
                .execution_for(owner_id)?
                .runtime
                .as_ref()
                .cloned()
                .ok_or_else(|| {
                    ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
                })?;
            let mut runtime = runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if runtime
                .cva
                .sync_principal_profile(&principal_id, &profile, perception_flow::now_ns())
                .map_err(operation)?
            {
                runtime.cva.sync().map_err(operation)?;
            }
        }

        self.deactivate_current()?;
        self.active_key = Some(key.clone());
        let active = self.executions.get(&key).expect("validated mounted REL");
        active.phylactery_enabled.store(true, Ordering::SeqCst);
        active.insomnia_enabled.store(
            self.active_insomnia_enabled.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
        {
            let mut queue = active
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            queue.clear(perception_queue::PerceptionOwner::User);
            queue.push_all(perception_queue::PerceptionOwner::User, user_keys);
        }
        notify_work(&active.signal)?;
        Ok(true)
    }

    fn activation_phy_state(
        &self,
    ) -> Result<
        (
            Option<(String, crate::PhylacteryProfile)>,
            Vec<crate::MemoryEntityMentionKey>,
        ),
        ReliquaryRuntimeHostError,
    > {
        let mut slot = self
            .phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let principal = slot
            .as_ref()
            .and_then(|phy| phy.owner_id().map(|id| (id, phy.profile())));
        let keys = match slot.as_mut() {
            Some(phy) => perception_owner::startup_keys_user(phy)?,
            None => Vec::new(),
        };
        Ok((principal, keys))
    }

    fn deactivate_current(&self) -> Result<(), ReliquaryRuntimeHostError> {
        let Some(old_key) = self.active_key.as_ref() else {
            return Ok(());
        };
        let Some(old) = self.executions.get(old_key) else {
            return Ok(());
        };
        old.insomnia_enabled.store(false, Ordering::SeqCst);
        old.phylactery_enabled.store(false, Ordering::SeqCst);
        old.memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .user = None;
        old.perception_queue
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clear(perception_queue::PerceptionOwner::User);
        notify_work(&old.signal)
    }
}
