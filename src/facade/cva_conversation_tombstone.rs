use crate::{
    ArchiveError, ConversationTombstone,
    owner_lifecycle::{LifecycleError, LifecycleState, LifecycleTarget, TombstoneCoordinates},
};
impl crate::Cva {
    /// Publish one terminal, direct-only tombstone; an exact retry returns `false`.
    pub fn tombstone_conversation(&mut self, conversation_id: &str) -> Result<bool, ArchiveError> {
        self.archive
            .tombstone_conversation(&mut self.container, conversation_id)
    }

    pub fn conversation_tombstone(&self, conversation_id: &str) -> Option<&ConversationTombstone> {
        self.archive.conversation_tombstone(conversation_id)
    }

    /// Inspect current versus terminal Archive lifecycle using Archive's existing clocks.
    pub fn conversation_lifecycle_state(
        &self,
        conversation_id: &str,
    ) -> Result<LifecycleState, LifecycleError> {
        if let Some(tombstone) = self.archive.conversation_tombstone(conversation_id) {
            return Ok(LifecycleState::Tombstoned(TombstoneCoordinates {
                global_version: Some(tombstone.global_version),
                domain_version: Some(tombstone.archive_version),
            }));
        }
        if self.archive.has_historical_conversation(conversation_id) {
            return Ok(LifecycleState::Active);
        }
        Err(LifecycleError::UnknownTarget(
            LifecycleTarget::Conversation(conversation_id.to_owned()),
        ))
    }
}
