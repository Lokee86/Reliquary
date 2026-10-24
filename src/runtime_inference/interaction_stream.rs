use crate::interaction_session::{InFlightMessage, validate_id};
use crate::{
    EpisodePolicy, InteractionAttachment, InteractionCompletion, InteractionError, InteractionRole,
    InteractionTurn, RelExecution,
};

impl RelExecution {
    pub fn begin_message(
        &mut self,
        session_id: &str,
        message_id: String,
        role: InteractionRole,
        timestamp_ns: i64,
    ) -> Result<(), InteractionError> {
        self.begin_message_with_principal(session_id, message_id, role, None, timestamp_ns)
    }

    pub(crate) fn begin_message_with_principal(
        &mut self,
        session_id: &str,
        message_id: String,
        role: InteractionRole,
        principal_id: Option<String>,
        timestamp_ns: i64,
    ) -> Result<(), InteractionError> {
        validate_id(&message_id, "message id")?;
        if self.cva.archive.has_node(session_id, &message_id) {
            return Err(InteractionError::MessageAlreadyDurable);
        }
        let parent_message_id = self
            .sessions
            .get(session_id)
            .ok_or(InteractionError::UnknownSession)?
            .leaf_message_id
            .clone();
        let principal_id = if role == InteractionRole::Agent {
            match parent_message_id.as_deref() {
                Some(parent_id) => self
                    .cva
                    .archive
                    .nodes
                    .get(session_id, parent_id)
                    .map(|node| node.principal_id.clone())
                    .unwrap_or(principal_id),
                None => principal_id,
            }
        } else {
            principal_id
        };
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::UnknownSession)?;
        if state.in_flight.is_some() {
            return Err(InteractionError::MessageInProgress);
        }
        state.in_flight = Some(InFlightMessage {
            message_id,
            parent_message_id,
            role,
            principal_id,
            timestamp_ns,
            content: String::new(),
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        });
        Ok(())
    }

    pub fn append_text(
        &mut self,
        session_id: &str,
        message_id: &str,
        delta: &str,
    ) -> Result<(), InteractionError> {
        self.message_mut(session_id, message_id)?
            .content
            .push_str(delta);
        Ok(())
    }

    pub fn attach(
        &mut self,
        session_id: &str,
        message_id: &str,
        attachment: InteractionAttachment,
    ) -> Result<(), InteractionError> {
        self.message_mut(session_id, message_id)?
            .attachments
            .push(attachment);
        Ok(())
    }

    pub fn cancel_message(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), InteractionError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::UnknownSession)?;
        let active = state
            .in_flight
            .as_ref()
            .ok_or(InteractionError::NoMessageInProgress)?;
        if active.message_id != message_id {
            return Err(InteractionError::MessageMismatch);
        }
        state.in_flight = None;
        Ok(())
    }

    pub fn complete_live_message(
        &mut self,
        session_id: &str,
        message_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<InteractionCompletion, InteractionError> {
        let receipt = self.complete_message(session_id, message_id)?;
        let scheduling = self.schedule_session_operation(session_id, policy, now_ns);
        Ok(InteractionCompletion {
            receipt,
            scheduling,
        })
    }

    pub fn complete_message(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<crate::InteractionReceipt, InteractionError> {
        let message = self.take_message(session_id, message_id)?;
        let turn = InteractionTurn {
            message_id: message.message_id.clone(),
            session_id: session_id.to_owned(),
            parent_message_id: message.parent_message_id.clone(),
            role: message.role,
            principal_id: message.principal_id.clone(),
            timestamp_ns: message.timestamp_ns,
            content: message.content.clone(),
            attachments: message.attachments.clone(),
            project_attachments: message.project_attachments.clone(),
        };
        match self.accept_turn_operation(turn) {
            Ok(receipt) => {
                self.sessions
                    .get_mut(session_id)
                    .ok_or(InteractionError::UnknownSession)?
                    .leaf_message_id = Some(message.message_id);
                Ok(receipt)
            }
            Err(error) => {
                if let Some(state) = self.sessions.get_mut(session_id) {
                    state.in_flight = Some(message);
                }
                Err(error.into())
            }
        }
    }

    pub(crate) fn take_message(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<InFlightMessage, InteractionError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::UnknownSession)?;
        let active = state
            .in_flight
            .as_ref()
            .ok_or(InteractionError::NoMessageInProgress)?;
        if active.message_id != message_id {
            return Err(InteractionError::MessageMismatch);
        }
        state
            .in_flight
            .take()
            .ok_or(InteractionError::NoMessageInProgress)
    }

    pub(crate) fn message_mut(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<&mut InFlightMessage, InteractionError> {
        let message = self
            .sessions
            .get_mut(session_id)
            .ok_or(InteractionError::UnknownSession)?
            .in_flight
            .as_mut()
            .ok_or(InteractionError::NoMessageInProgress)?;
        if message.message_id != message_id {
            return Err(InteractionError::MessageMismatch);
        }
        Ok(message)
    }
}
