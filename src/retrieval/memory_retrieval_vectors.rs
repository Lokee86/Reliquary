use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorReadView;
use crate::{CompatibilityProfileId, MemoryBodyId, MemoryRetrievalError};
use std::collections::HashMap;

pub(crate) fn load_memory_vectors(
    memories: &MemoryReadView<'_>,
    memory_vectors: &MemoryVectorReadView<'_>,
    profile_id: CompatibilityProfileId,
) -> Result<HashMap<MemoryBodyId, Vec<f32>>, MemoryRetrievalError> {
    crate::memory_vector_reader::MemoryVectorReader::new(*memory_vectors)
        .load(memories, profile_id)
        .map_err(|error| match error {
            crate::memory_vector_reader::MemoryVectorReadError::MemoryVectors(error) => {
                MemoryRetrievalError::MemoryVectors(error)
            }
            crate::memory_vector_reader::MemoryVectorReadError::PackedVectors(error) => {
                MemoryRetrievalError::PackedVectors(error)
            }
            crate::memory_vector_reader::MemoryVectorReadError::UnsupportedScalar(scalar) => {
                MemoryRetrievalError::UnsupportedScalar(scalar)
            }
            crate::memory_vector_reader::MemoryVectorReadError::CorruptVector(reason) => {
                MemoryRetrievalError::CorruptVector(reason)
            }
        })
}

/// Canonical exact scorer, before routing, seed or presentation limits.
/// Candidates are explicit so passive routed retrieval shares identical scoring.
pub(crate) fn exact_memory_scores(
    query: &[f32],
    vectors: &HashMap<crate::MemoryId, Vec<f32>>,
    candidates: &[crate::MemoryId],
) -> Result<Vec<crate::MemoryRetrievalHit>, MemoryRetrievalError> {
    validate_query(query)?;
    let mut ranked = Vec::with_capacity(candidates.len());
    for id in candidates {
        let vector = vectors.get(id).ok_or(MemoryRetrievalError::CorruptVector(
            "missing candidate vector",
        ))?;
        let score = crate::compatibility_vector::cosine(query, vector)
            .ok_or(MemoryRetrievalError::InvalidQueryVector)?;
        ranked.push(crate::MemoryRetrievalHit {
            memory_id: *id,
            score,
        });
    }
    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    Ok(ranked)
}

pub(crate) fn validate_query(query: &[f32]) -> Result<(), MemoryRetrievalError> {
    if query.is_empty() || query.iter().any(|value| !value.is_finite()) {
        return Err(MemoryRetrievalError::InvalidQueryVector);
    }
    let norm = query
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>();
    if norm == 0.0 {
        return Err(MemoryRetrievalError::InvalidQueryVector);
    }
    Ok(())
}

/// Load the current searchable population using the same validation for both consumers.
pub(crate) fn load_searchable_memory_vectors(
    memories: &MemoryReadView<'_>,
    memory_vectors: &MemoryVectorReadView<'_>,
    profile_id: CompatibilityProfileId,
) -> Result<
    (
        std::collections::HashSet<crate::MemoryId>,
        HashMap<crate::MemoryId, Vec<f32>>,
    ),
    MemoryRetrievalError,
> {
    let by_body = load_memory_vectors(memories, memory_vectors, profile_id)?;
    let mut searchable = std::collections::HashSet::new();
    let mut vectors = HashMap::new();
    for id in memories.current_ids() {
        let memory = memories.memory(id)?;
        if memory.archived {
            continue;
        }
        searchable.insert(id);
        let body_id = memories.current_body_id(id)?;
        if let Some(vector) = by_body.get(&body_id) {
            if crate::compatibility_vector::cosine(vector, vector).is_none() {
                return Err(MemoryRetrievalError::CorruptVector("invalid stored vector"));
            }
            vectors.insert(id, vector.clone());
        }
    }
    if let Some(dimensions) = vectors.values().next().map(Vec::len) {
        if vectors.values().any(|vector| vector.len() != dimensions) {
            return Err(MemoryRetrievalError::CorruptVector("vector dimensions"));
        }
    }
    Ok((searchable, vectors))
}
