use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, FragmentId, SemanticSearchError,
    SemanticSearchHit,
};
use std::collections::HashSet;

impl Cva {
    pub fn semantic_search(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SemanticSearchHit>, SemanticSearchError> {
        self.runtime_semantic_search(compatibility_profile_id, endpoint, query, limit)
    }

    pub(crate) fn semantic_search_vector(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
        allowed_fragments: Option<&HashSet<FragmentId>>,
    ) -> Result<Vec<SemanticSearchHit>, SemanticSearchError> {
        self.runtime_semantic_search_vector(
            compatibility_profile_id,
            query,
            limit,
            allowed_fragments,
        )
    }
}
