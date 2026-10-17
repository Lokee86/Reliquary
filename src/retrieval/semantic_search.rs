use crate::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode, FragmentId,
    MAX_SEMANTIC_SEARCH_LIMIT, ScalarType, SemanticSearchError, SemanticSearchHit,
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
        if !(1..=MAX_SEMANTIC_SEARCH_LIMIT).contains(&limit) {
            return Err(SemanticSearchError::InvalidLimit);
        }
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
        let packed = self
            .packed_vectors
            .get(&mut self.container, binding.packed_vector_id)?;
        let schema = packed.schema();
        if schema.scalar != ScalarType::F32 {
            return Err(SemanticSearchError::UnsupportedScalar(schema.scalar));
        }
        if usize::try_from(schema.dimensions).ok() != Some(query.len()) {
            return Err(SemanticSearchError::InvalidQueryVector);
        }
        if packed.count() != binding.fragment_ids.len() {
            return Err(SemanticSearchError::CorruptMatrix("row binding count"));
        }
        let query_norm = vector_norm(query).ok_or(SemanticSearchError::InvalidQueryVector)?;
        let mut scores = Vec::with_capacity(packed.count());
        for ordinal in 0..packed.count() {
            let fragment_id = *binding
                .fragment_ids
                .get(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("row binding"))?;
            if allowed_fragments.is_some_and(|allowed| !allowed.contains(&fragment_id)) {
                continue;
            }
            let row = packed
                .row(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("missing row"))?;
            if let Some(score) = cosine_f32_row(row, query, query_norm)?
                && score > 0.0
            {
                scores.push((ordinal, score));
            }
        }
        scores.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        scores.truncate(limit);

        let mut hits = Vec::with_capacity(scores.len());
        for (ordinal, score) in scores {
            let fragment_id = *binding
                .fragment_ids
                .get(ordinal)
                .ok_or(SemanticSearchError::CorruptMatrix("row binding"))?;
            let fragment = self
                .archive
                .fragments
                .get(fragment_id)
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

fn vector_norm(vector: &[f32]) -> Option<f64> {
    let mut norm = 0.0_f64;
    for value in vector {
        let value = *value as f64;
        if !value.is_finite() {
            return None;
        }
        norm += value * value;
    }
    (norm > 0.0).then(|| norm.sqrt())
}

fn cosine_f32_row(
    row: &[u8],
    query: &[f32],
    query_norm: f64,
) -> Result<Option<f64>, SemanticSearchError> {
    if row.len() != query.len().saturating_mul(4) {
        return Err(SemanticSearchError::CorruptMatrix("f32 row width"));
    }
    let mut dot = 0.0_f64;
    let mut row_norm_squared = 0.0_f64;
    for (encoded, query_value) in row.chunks_exact(4).zip(query) {
        let value = f32::from_le_bytes(encoded.try_into().expect("f32 width")) as f64;
        if !value.is_finite() {
            return Err(SemanticSearchError::CorruptMatrix("non-finite row value"));
        }
        dot += value * (*query_value as f64);
        row_norm_squared += value * value;
    }
    if row_norm_squared == 0.0 {
        return Ok(None);
    }
    let score = dot / (query_norm * row_norm_squared.sqrt());
    if !score.is_finite() {
        return Err(SemanticSearchError::CorruptMatrix("non-finite cosine"));
    }
    Ok(Some(score))
}
