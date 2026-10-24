use crate::{ArchiveError, ConversationSummary, Cva, ResolvedTurn};

impl Cva {
    pub fn conversation_summaries(&self) -> Vec<ConversationSummary> {
        self.archive.conversation_summaries()
    }

    pub fn conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.archive.conversation_turns(
            &mut self.owner.container,
            conversation_id,
            leaf_node_id,
        )
    }

    pub fn historical_conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.owner.archive.historical_conversation_turns(
            &mut self.owner.container,
            conversation_id,
            leaf_node_id,
        )
    }

    pub fn conversation_turn_page(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
    ) -> Result<(Vec<ResolvedTurn>, Option<String>), ArchiveError> {
        self.owner.archive.conversation_turn_page(
            &mut self.owner.container,
            conversation_id,
            end_node_id,
            limit,
        )
    }

    pub fn conversation_branch_start_node_ids(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<String>, ArchiveError> {
        self.archive
            .conversation_branch_start_node_ids(conversation_id, leaf_node_id)
    }
}
