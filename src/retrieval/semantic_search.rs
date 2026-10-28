use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode, FragmentId,
    MAX_SEMANTIC_SEARCH_LIMIT, ScalarType, SemanticSearchError, SemanticSearchHit,
};
use reliquary_retrieval::semantic_scoring::{
    SemanticScoringError, cosine_f32_row, rank_scores, validate_query_dimensions,
    validate_semantic_limit, validate_semantic_request, vector_norm,
};
use reliquary_vectors::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
use std::collections::HashSet;

impl Cva {
    pub fn semantic_search(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SemanticSearchHit>, SemanticSearchError> {
        validate_semantic_request(query, limit, MAX_SEMANTIC_SEARCH_LIMIT)
            .map_err(map_scoring_error)?;
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
        validate_semantic_limit(limit, MAX_SEMANTIC_SEARCH_LIMIT).map_err(map_scoring_error)?;
        self.search_current_generation(compatibility_profile_id, query, limit, allowed_fragments)
    }

    fn search_current_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
        allowed_fragments: Option<&HashSet<FragmentId>>,
    ) -> Result<Vec<SemanticSearchHit>, SemanticSearchError> {
        let generation = self
            .vector_generations
            .current_generation(compatibility_profile_id)
            .ok_or(SemanticSearchError::MissingGeneration)?;
        let binding = self
            .archive_vectors
            .get(&mut self.container, generation.archive_vector_id)?;
        let fragment_ids: Vec<_> = binding
            .fragment_ids
            .into_iter()
            .map(|id| crate::FragmentId(id.0))
            .collect();
        let packed = self
            .packed_vectors
            .get(&mut self.container, binding.packed_vector_id)?;
        let schema = packed.schema();
        if schema.scalar != ScalarType::F32 {
            return Err(SemanticSearchError::UnsupportedScalar(schema.scalar));
        }
        validate_query_dimensions(query, schema.dimensions).map_err(map_scoring_error)?;
        if packed.count() != fragment_ids.len() {
            return Err(SemanticSearchError::CorruptMatrix("row binding count"));
        }
        let query_norm = vector_norm(query).ok_or(SemanticSearchError::InvalidQueryVector)?;
        let mut scores = Vec::with_capacity(packed.count());
        for ordinal in 0..packed.count() {
            let fragment_id = *fragment_ids
                .get(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("row binding"))?;
            if allowed_fragments.is_some_and(|allowed| !allowed.contains(&fragment_id)) {
                continue;
            }
            let row = packed
                .row(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("missing row"))?;
            if let Some(score) = cosine_f32_row(row, query, query_norm)
                .map_err(SemanticSearchError::CorruptMatrix)?
            {
                scores.push((ordinal, score));
            }
        }
        let scores = rank_scores(scores, limit);

        let mut hits = Vec::with_capacity(scores.len());
        for (ordinal, score) in scores {
            let fragment_id = *fragment_ids
                .get(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("row binding"))?;
            let fragment = self
                .archive
                .source_fragment(fragment_id)
                .cloned()
                .ok_or(SemanticSearchError::MissingFragment)?;
            hits.push(SemanticSearchHit {
                fragment,
                score,
                ordinal: u64::try_from(ordinal)
                    .map_err(|_| SemanticSearchError::CorruptMatrix("row ordinal"))?,
                generation_id: generation.id,
            });
        }
        Ok(hits)
    }
}

fn map_scoring_error(error: SemanticScoringError) -> SemanticSearchError {
    match error {
        SemanticScoringError::EmptyQuery => SemanticSearchError::EmptyQuery,
        SemanticScoringError::InvalidLimit => SemanticSearchError::InvalidLimit,
        SemanticScoringError::InvalidQueryVector => SemanticSearchError::InvalidQueryVector,
    }
}
