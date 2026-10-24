use crate::compatibility_profile_probe::validate_embedding_batch;
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId, MemoryBodyId,
    MemoryVectorError, PackedVectors, ScalarType, VectorSchema,
};

pub(crate) struct RuntimeMemoryVectorBatch {
    pub(crate) profile: CompatibilityProfile,
    pub(crate) body_ids: Vec<MemoryBodyId>,
    pub(crate) texts: Vec<String>,
}
pub(crate) fn accept_profile(
    container: &mut crate::Container,
    profiles: &mut crate::compatibility_profile_store::CompatibilityProfileStore,
    candidate: CompatibilityProfile,
) -> Result<CompatibilityProfile, CompatibilityProfileError> {
    if let Some(existing) = profiles.compatible_with(&candidate) {
        return Ok(existing.clone());
    }
    profiles.put(container, candidate.clone())?;
    Ok(candidate)
}

pub(crate) fn prepare_batch(
    container: &mut crate::Container,
    memories: &crate::memory_store::MemoryStore,
    profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    memory_vectors: &crate::memory_vector_store::MemoryVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
    let profile = profiles
        .get_active(profile_id)
        .cloned()
        .ok_or(MemoryVectorError::MissingProfile)?;
    let current = memories.current_body_ids();
    if current.is_empty() {
        return Ok(None);
    }
    let body_ids = memory_vectors.missing_body_ids(profile_id, &current);
    if body_ids.is_empty() {
        return Ok(None);
    }
    let mut texts = Vec::with_capacity(body_ids.len());
    for body_id in &body_ids {
        texts.push(memories.body_embedding_text(container, *body_id)?);
    }
    Ok(Some(RuntimeMemoryVectorBatch {
        profile,
        body_ids,
        texts,
    }))
}

pub(crate) fn commit_batch(
    container: &mut crate::Container,
    memories: &crate::memory_store::MemoryStore,
    profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    packed_vectors: &mut crate::packed_vector_store::PackedVectorStore,
    memory_vectors: &mut crate::memory_vector_store::MemoryVectorStore,
    batch: RuntimeMemoryVectorBatch,
    vectors: Vec<Vec<f32>>,
) -> Result<usize, MemoryVectorError> {
    if profiles.get_active(batch.profile.id).is_none() {
        return Err(MemoryVectorError::MissingProfile);
    }
    validate_embedding_batch(&batch.profile, &vectors, batch.body_ids.len())?;
    let mut body_ids = Vec::with_capacity(batch.body_ids.len());
    let mut pending_vectors = Vec::with_capacity(vectors.len());
    for (body_id, vector) in batch.body_ids.into_iter().zip(vectors) {
        if memory_vectors.location(batch.profile.id, body_id).is_none() {
            body_ids.push(body_id);
            pending_vectors.push(vector);
        }
    }
    if body_ids.is_empty() {
        return Ok(0);
    }
    if body_ids
        .iter()
        .any(|body_id| !memories.contains_body(*body_id))
    {
        return Err(MemoryVectorError::MissingMemoryBody);
    }
    let schema = VectorSchema::new(batch.profile.dimensions, ScalarType::F32)
        .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
    let row_bytes = usize::try_from(batch.profile.dimensions)
        .ok()
        .and_then(|dimensions| dimensions.checked_mul(4))
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
    let packed = PackedVectors::from_bytes(schema, bytes)
        .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
    let packed_vector_id = packed_vectors.put(container, packed)?;
    memory_vectors.put(
        container,
        profiles,
        packed_vectors,
        batch.profile.id,
        packed_vector_id,
        body_ids.clone(),
    )?;
    Ok(body_ids.len())
}
