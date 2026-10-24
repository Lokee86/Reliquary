use crate::interaction_session::InFlightMessage;
use crate::{InteractionError, InteractionStreamRecord, InteractionStreamStatus, RelExecution};

impl RelExecution {
    pub fn checkpoint_message(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), InteractionError> {
        let record = {
            let message = self.message_mut(session_id, message_id)?;
            stream_record(session_id, message, InteractionStreamStatus::Streaming)
        };
        self.cva
            .owner
            .interaction_streams
            .put(&mut self.cva.owner.container, record)
            .map_err(crate::CvaError::from)?;
        self.cva.sync_owner()?;
        Ok(())
    }

    pub fn append_checkpointed_text(
        &mut self,
        session_id: &str,
        message_id: &str,
        delta: &str,
    ) -> Result<(), InteractionError> {
        self.message_mut(session_id, message_id)?
            .content
            .push_str(delta);
        self.checkpoint_message(session_id, message_id)
    }

    pub fn interrupt_message(
        &mut self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), InteractionError> {
        let message = self.take_message(session_id, message_id)?;
        if message.content.is_empty() {
            return Ok(());
        }
        let record = stream_record(session_id, &message, InteractionStreamStatus::Interrupted);
        if let Err(error) = self
            .cva
            .owner
            .interaction_streams
            .put(&mut self.cva.owner.container, record)
            .map_err(crate::CvaError::from)
            .and_then(|_| self.cva.sync_owner())
        {
            if let Some(state) = self.sessions.get_mut(session_id) {
                state.in_flight = Some(message);
            }
            return Err(error.into());
        }
        Ok(())
    }
}

fn stream_record(
    session_id: &str,
    message: &InFlightMessage,
    status: InteractionStreamStatus,
) -> InteractionStreamRecord {
    InteractionStreamRecord {
        message_id: message.message_id.clone(),
        session_id: session_id.to_owned(),
        parent_message_id: message.parent_message_id.clone(),
        role: message.role,
        timestamp_ns: message.timestamp_ns,
        content: message.content.clone(),
        status,
    }
}
