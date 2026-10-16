use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    CompatibilityProfileId, Container, MemoryBodyId, MemoryRetrievalError, MemoryVectorId,
    ScalarType,
};
use std::collections::HashMap;

pub(crate) fn load_memory_vectors(
    container: &mut Container,
    memories: &MemoryStore,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<HashMap<MemoryBodyId, Vec<f32>>, MemoryRetrievalError> {
    let mut by_set: HashMap<MemoryVectorId, Vec<(MemoryBodyId, u64)>> = HashMap::new();
    for body_id in memories.current_body_ids() {
        if let Some(location) = memory_vectors.location(profile_id, body_id) {
            by_set
                .entry(location.set_id)
                .or_default()
                .push((body_id, location.row));
        }
    }
    let mut output = HashMap::new();
    for (set_id, locations) in by_set {
        let set = memory_vectors.get(container, set_id)?;
        let packed = packed_vectors.get(container, set.packed_vector_id)?;
        if packed.schema().scalar != ScalarType::F32 {
            return Err(MemoryRetrievalError::UnsupportedScalar(
                packed.schema().scalar,
            ));
        }
        for (body_id, row) in locations {
            let ordinal = usize::try_from(row)
                .map_err(|_| MemoryRetrievalError::CorruptVector("row ordinal"))?;
            let bytes = packed
                .row(ordinal)
                .ok_or(MemoryRetrievalError::CorruptVector("missing row"))?;
            output.insert(body_id, decode_f32_row(bytes)?);
        }
    }
    Ok(output)
}

fn decode_f32_row(bytes: &[u8]) -> Result<Vec<f32>, MemoryRetrievalError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(MemoryRetrievalError::CorruptVector("f32 row width"));
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let value = f32::from_le_bytes(chunk.try_into().expect("f32 width"));
            value
                .is_finite()
                .then_some(value)
                .ok_or(MemoryRetrievalError::CorruptVector("non-finite vector"))
        })
        .collect()
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
