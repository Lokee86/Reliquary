use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::{
    CompatibilityProfileId, DreamCandidateError, MemoryBodyId, MemoryVectorId, ScalarType,
};
use reliquary_container::Container;
use reliquary_vector_store::PackedVectorStore;
use std::collections::HashMap;

pub(crate) fn load_memory_vectors(
    container: &mut Container,
    memories: &MemoryStore,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<HashMap<MemoryBodyId, Vec<f32>>, DreamCandidateError> {
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
            return Err(DreamCandidateError::UnsupportedScalar(
                packed.schema().scalar,
            ));
        }
        for (body_id, row) in locations {
            let ordinal = usize::try_from(row)
                .map_err(|_| DreamCandidateError::CorruptVector("row ordinal"))?;
            let bytes = packed
                .row(ordinal)
                .ok_or(DreamCandidateError::CorruptVector("missing row"))?;
            output.insert(body_id, decode_f32_row(bytes)?);
        }
    }
    Ok(output)
}

fn decode_f32_row(bytes: &[u8]) -> Result<Vec<f32>, DreamCandidateError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(DreamCandidateError::CorruptVector("f32 row width"));
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let value = f32::from_le_bytes(chunk.try_into().expect("f32 width"));
            value
                .is_finite()
                .then_some(value)
                .ok_or(DreamCandidateError::CorruptVector("non-finite vector"))
        })
        .collect()
}
