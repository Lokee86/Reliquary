use crate::fragmenter::path_fragment_windows;
use crate::{
    CompatibilityProfileId, ConversationSearchHit, Cva, FragmentConfig, RetrievalConfig,
    SearchError,
};

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
        if !(1..=crate::MAX_SEMANTIC_SEARCH_LIMIT).contains(&limit) {
            return Err(SearchError::InvalidConfig);
        }
        self.archive
            .require_current_conversation_id(conversation_id)?;
        let nodes = self.archive.branch_nodes(conversation_id, leaf_node_id)?;
        let fragments = path_fragment_windows(&nodes, FragmentConfig::default(), true)?;
        let candidate_limit = crate::DEFAULT_SEARCH_CANDIDATE_LIMIT
            .max(limit)
            .min(crate::MAX_SEMANTIC_SEARCH_LIMIT);
        let candidates = self.search_fragments_with_vector(
            compatibility_profile_id,
            query_vector,
            query,
            &fragments,
            RetrievalConfig {
                candidate_limit,
                result_limit: limit,
                ..RetrievalConfig::default()
            },
        )?;
        candidates
            .into_iter()
            .map(|candidate| {
                let text = self
                    .archive
                    .fragment_text_for(&mut self.container, &candidate.fragment)?;
                Ok(ConversationSearchHit {
                    fragment: candidate.fragment,
                    text,
                    score: candidate.combined_score,
                })
            })
            .collect()
    }
}
