use crate::compatibility_profile_probe::validate_embedding_batch;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::{MemoryVectorPublicationPlan, MemoryVectorStore};
use crate::{
    CompatibilityProfile, CompatibilityProfileId, MemoryBodyId, MemoryVectorError, PackedVectors,
    ScalarType, VectorSchema,
};

pub(crate) struct RuntimeMemoryVectorBatch {
    pub(crate) profile: CompatibilityProfile,
    pub(crate) body_ids: Vec<MemoryBodyId>,
    pub(crate) texts: Vec<String>,
}

pub(crate) fn publication_plan(
    memories: &MemoryStore,
    profiles: &CompatibilityProfileStore,
    memory_vectors: &MemoryVectorStore,
    batch: RuntimeMemoryVectorBatch,
    vectors: Vec<Vec<f32>>,
) -> Result<Option<MemoryVectorPublicationPlan>, MemoryVectorError> {
    if profiles.get_active(batch.profile.id).is_none() {
        return Err(MemoryVectorError::MissingProfile);
    }
    validate_embedding_batch(&batch.profile, &vectors, batch.body_ids.len())?;
    let mut body_ids = Vec::with_capacity(batch.body_ids.len());
    let mut pending_vectors = Vec::with_capacity(vectors.len());
    for (body, vector) in batch.body_ids.into_iter().zip(vectors) {
        if memory_vectors.location(batch.profile.id, body).is_none() {
            body_ids.push(body);
            pending_vectors.push(vector);
        }
    }
    if body_ids.is_empty() {
        return Ok(None);
    }
    if body_ids.iter().any(|id| !memories.contains_body(*id)) {
        return Err(MemoryVectorError::MissingMemoryBody);
    }
    let schema = VectorSchema::new(batch.profile.dimensions, ScalarType::F32)
        .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
    let row_bytes = usize::try_from(batch.profile.dimensions)
        .ok()
        .and_then(|d| d.checked_mul(4))
        .ok_or(MemoryVectorError::InvalidEmbeddingMatrix)?;
    let capacity = pending_vectors
        .len()
        .checked_mul(row_bytes)
        .ok_or(MemoryVectorError::InvalidEmbeddingMatrix)?;
    let mut bytes = Vec::with_capacity(capacity);
    for vector in pending_vectors {
        for value in vector {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    let packed_vectors = PackedVectors::from_bytes(schema, bytes)
        .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
    Ok(Some(MemoryVectorPublicationPlan {
        profile_id: batch.profile.id,
        packed_vectors,
        body_ids,
    }))
}
