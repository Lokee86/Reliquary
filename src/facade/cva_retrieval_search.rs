use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, Fragment, RetrievalConfig, SearchCandidate,
    SearchError,
};

impl Cva {
    pub fn search(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        self.runtime_search_with_config(
            compatibility_profile_id,
            endpoint,
            query,
            RetrievalConfig::default(),
        )
    }

    pub fn search_with_result_limit(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
        result_limit: usize,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        let candidate_limit = crate::DEFAULT_SEARCH_CANDIDATE_LIMIT
            .max(result_limit)
            .min(crate::MAX_SEMANTIC_SEARCH_LIMIT);
        self.runtime_search_with_config(
            compatibility_profile_id,
            endpoint,
            query,
            RetrievalConfig {
                candidate_limit,
                result_limit,
                ..RetrievalConfig::default()
            },
        )
    }

    pub fn search_with_config(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
        config: RetrievalConfig,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        self.runtime_search_with_config(compatibility_profile_id, endpoint, query, config)
    }

    pub(crate) fn search_fragments_with_vector(
        &mut self,
        compatibility_profile_id: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
        query: &str,
        fragments: &[Fragment],
        config: RetrievalConfig,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        self.runtime_search_fragments_with_vector(
            compatibility_profile_id,
            query_vector,
            query,
            fragments,
            config,
        )
    }
}
