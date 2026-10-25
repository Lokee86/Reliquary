use crate::{
    ArchiveError, ConversationTombstone,
    owner_lifecycle::{LifecycleError, LifecycleState},
};
impl crate::Cva {
    /// Publish one terminal, direct-only tombstone; an exact retry returns `false`.
    pub fn tombstone_conversation(&mut self, conversation_id: &str) -> Result<bool, ArchiveError> {
        self.native_tombstone_conversation(conversation_id)
    }

    pub fn conversation_tombstone(&self, conversation_id: &str) -> Option<&ConversationTombstone> {
        self.native_conversation_tombstone(conversation_id)
    }

    /// Inspect current versus terminal Archive lifecycle using Archive's existing clocks.
    pub fn conversation_lifecycle_state(
        &self,
        conversation_id: &str,
    ) -> Result<LifecycleState, LifecycleError> {
        self.native_conversation_lifecycle_state(conversation_id)
    }
}
