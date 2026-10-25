use crate::{CompatibilityProfileId, ConversationSearchHit, Cva, SearchError};

impl Cva {
    pub fn search_conversation_branch(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, SearchError> {
        self.runtime_search_conversation_branch(
            conversation_id,
            leaf_node_id,
            query,
            limit,
            None,
            None,
        )
    }

    pub(crate) fn search_conversation_branch_with_vector(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        query_vector: &[f32],
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, SearchError> {
        self.runtime_search_conversation_branch(
            conversation_id,
            leaf_node_id,
            query,
            limit,
            Some(compatibility_profile_id),
            Some(query_vector),
        )
    }
}
