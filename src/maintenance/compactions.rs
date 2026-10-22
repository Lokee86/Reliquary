//! Explicit dispatch for owner-local stale conversation-compaction recovery.
use crate::container::Container;
use crate::conversation_compaction_store::ConversationCompactionStore;
use crate::{ConversationCompactionError, ConversationCompactionMaintenance};

pub(crate) fn maintain_conversation_compactions(
    container: &mut Container,
    compactions: &mut ConversationCompactionStore,
) -> Result<ConversationCompactionMaintenance, ConversationCompactionError> {
    compactions.recover(container)
}
