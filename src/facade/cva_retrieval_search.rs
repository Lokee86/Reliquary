use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, Fragment, FragmentId, RetrievalConfig,
    SearchCandidate, SearchError,
};
use std::collections::HashSet;

impl Cva {
    #[cfg(test)]
    pub(crate) fn diversify_candidates(
        &self,
        candidates: Vec<SearchCandidate>,
        limit: usize,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        crate::search_rank::diversify_candidates(&self.archive, candidates, limit)
    }

    pub fn search(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        self.search_with_config(
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
        self.search_with_config(
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
        let weights = crate::search::validate_search(query, config)?;
        let lexical = self.lexical_candidates(query, config.candidate_limit)?;
        let semantic = self.semantic_search(
            compatibility_profile_id,
            endpoint,
            query,
            config.candidate_limit,
        )?;
        crate::search_rank::rank_search_candidates(
            &self.archive,
            lexical,
            semantic,
            config,
            weights,
        )
    }

    pub(crate) fn search_fragments_with_vector(
        &mut self,
        compatibility_profile_id: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
        query: &str,
        fragments: &[Fragment],
        config: RetrievalConfig,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        let weights = crate::search::validate_search(query, config)?;
        let lexical =
            self.lexical_candidates_from_fragments(query, fragments, config.candidate_limit)?;
        let semantic = match (compatibility_profile_id, query_vector) {
            (Some(profile), Some(vector))
                if self
                    .vector_generations
                    .current_generation(profile)
                    .is_some() =>
            {
                let allowed: HashSet<FragmentId> =
                    fragments.iter().map(|fragment| fragment.id).collect();
                self.semantic_search_vector(
                    profile,
                    vector,
                    config.candidate_limit,
                    Some(&allowed),
                )?
            }
            _ => Vec::new(),
        };
        crate::search_rank::rank_search_candidates(
            &self.archive,
            lexical,
            semantic,
            config,
            weights,
        )
    }
}
