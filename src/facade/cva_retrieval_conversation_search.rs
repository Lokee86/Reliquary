use crate::{CompatibilityProfileId, ConversationSearchHit, Cva, SearchError};

impl Cva {
    pub fn search_conversation_branch(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, SearchError> {
        self.search_conversation_branch_inner(
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
        self.search_conversation_branch_inner(
            conversation_id,
            leaf_node_id,
            query,
            limit,
            Some(compatibility_profile_id),
            Some(query_vector),
        )
    }

    fn search_conversation_branch_inner(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
        compatibility_profile_id: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
    ) -> Result<Vec<ConversationSearchHit>, SearchError> {
        crate::conversation_search::search_conversation_branch_parts(
            &self.archive,
            &mut self.container,
            &mut self.lexical_index,
            &mut self.vector_generations,
            &mut self.archive_vectors,
            &mut self.packed_vectors,
            compatibility_profile_id,
            query_vector,
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }
}
