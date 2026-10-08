//! Untrusted durable reconnect hints over current P3/P4 authority.
use super::*;

impl LiveSession {
    /// Preserve a previously delivered snapshot as a reconnect hint.
    pub fn resume_token(
        &self,
        snapshot: &LiveStreamSnapshot,
    ) -> Result<LiveResumeToken, LiveRuntimeError> {
        self.validate()?;
        let current = self.native_generation(&snapshot.generation.operation_id)?;
        if snapshot.cursor.mount_epoch != self.token.mount_epoch
            || snapshot.generation.branch_id != self.branch_id
            || snapshot.generation.message_id != current.message_id
            || snapshot.generation.checkpoint_sequence > current.checkpoint_sequence
        {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        Ok(LiveResumeToken {
            owner_id: self.token.owner_id().to_owned(),
            conversation_id: self.conversation_id.clone(),
            branch_id: self.branch_id.clone(),
            operation_id: current.operation_id,
            message_id: current.message_id,
            cursor: snapshot.cursor,
            checkpoint_sequence: snapshot.generation.checkpoint_sequence,
        })
    }
    /// Reauthorize by opening a fresh session before using a stored hint.
    /// Always join at the current full prefix, without cursor replay or dispatch.
    pub fn resume_generation(
        &mut self,
        hint: &LiveResumeToken,
        capacity: usize,
    ) -> Result<LiveStreamSnapshot, LiveRuntimeError> {
        self.validate()?;
        if hint.cursor.mount_epoch != self.token.mount_epoch {
            return Err(LiveRuntimeError::FreshSessionRequired);
        }
        if hint.owner_id != self.token.owner_id()
            || hint.conversation_id != self.conversation_id
            || hint.branch_id != self.branch_id
        {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        let current = self.native_generation(&hint.operation_id)?;
        if hint.message_id != current.message_id
            || hint.checkpoint_sequence > current.checkpoint_sequence
        {
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        let snapshot = self.watch_generation(&hint.operation_id, capacity)?;
        if hint.cursor.delivery_sequence > snapshot.cursor.delivery_sequence {
            if let Some(stream) = self.stream.take() {
                let _ = self
                    .gateway
                    .disconnect_conversation_stream(&self.instance, &stream);
            }
            return Err(LiveRuntimeError::OperationUnavailable);
        }
        self.resync_required.store(false, Ordering::SeqCst);
        Ok(snapshot)
    }
}
