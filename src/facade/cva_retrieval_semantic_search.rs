use crate::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode, FragmentId,
    MAX_SEMANTIC_SEARCH_LIMIT, SemanticSearchError, SemanticSearchHit,
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
        if query.trim().is_empty() {
            return Err(SemanticSearchError::EmptyQuery);
        }
        if !(1..=MAX_SEMANTIC_SEARCH_LIMIT).contains(&limit) {
            return Err(SemanticSearchError::InvalidLimit);
        }
        let profile = self
            .compatibility_profiles
            .get_active(compatibility_profile_id)
            .cloned()
            .ok_or(SemanticSearchError::MissingProfile)?;
        if !verify_endpoint(&profile, endpoint)?.compatible {
            return Err(SemanticSearchError::IncompatibleEndpoint);
        }
        let vectors = endpoint.embed(EmbeddingMode::Query, &[query.to_string()])?;
        validate_embedding_batch(&profile, &vectors, 1)?;
        let query_vector = vectors
            .first()
            .ok_or(SemanticSearchError::InvalidQueryVector)?;
        self.semantic_search_vector(compatibility_profile_id, query_vector, limit, None)
    }

    pub(crate) fn semantic_search_vector(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
        allowed_fragments: Option<&HashSet<FragmentId>>,
    ) -> Result<Vec<SemanticSearchHit>, SemanticSearchError> {
        crate::semantic_search::semantic_search_vector(
            &mut self.vector_generations,
            &mut self.archive_vectors,
            &mut self.packed_vectors,
            &mut self.container,
            &self.archive,
            compatibility_profile_id,
            query,
            limit,
            allowed_fragments,
        )
    }
}
