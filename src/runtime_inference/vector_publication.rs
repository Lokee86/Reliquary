use crate::compatibility_profile_probe::validate_embedding_batch;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::runtime_vector_step_policy::{self, RuntimeMemoryVectorBatch};
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId, Container,
    MemoryBodyId, MemoryVectorError, PackedVectors, ScalarType, VectorSchema,
};

pub(crate) fn accept_profile(
    container: &mut Container,
    profiles: &mut CompatibilityProfileStore,
    candidate: CompatibilityProfile,
) -> Result<CompatibilityProfile, CompatibilityProfileError> {
    if let Some(existing) = profiles.compatible_with(&candidate) {
        return Ok(existing.clone());
    }
    profiles.put(container, candidate.clone())?;
    Ok(candidate)
}

pub(crate) fn prepare_batch(
    container: &mut Container,
    memories: &MemoryStore,
    profiles: &CompatibilityProfileStore,
    vectors: &MemoryVectorStore,
    profile_id: CompatibilityProfileId,
) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
    let profile = profiles
        .get_active(profile_id)
        .cloned()
        .ok_or(MemoryVectorError::MissingProfile)?;
    let current = memories.current_body_ids();
    let body_ids =
        runtime_vector_step_policy::select_body_ids(profile_id, &current, |profile, ids| {
            vectors.missing_body_ids(profile, ids)
        });
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
    container: &mut Container,
    memories: &MemoryStore,
    profiles: &CompatibilityProfileStore,
    packed_vectors: &mut PackedVectorStore,
    memory_vectors: &mut MemoryVectorStore,
    batch: RuntimeMemoryVectorBatch,
    vectors: Vec<Vec<f32>>,
) -> Result<usize, MemoryVectorError> {
    if profiles.get_active(batch.profile.id).is_none() {
        return Err(MemoryVectorError::MissingProfile);
    }
    validate_embedding_batch(&batch.profile, &vectors, batch.body_ids.len())?;
    let (body_ids, pending_vectors) = runtime_vector_step_policy::pending_rows(
        batch.profile.id,
        batch.body_ids,
        vectors,
        |profile, body| memory_vectors.location(profile, body).is_some(),
    );
    if body_ids.is_empty() {
        return Ok(0);
    }
    runtime_vector_step_policy::validate_body_presence(&body_ids, |id| memories.contains_body(id))?;
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
    let packed = PackedVectors::from_bytes(schema, bytes)
        .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
    let packed_id = packed_vectors.put(container, packed)?;
    memory_vectors.put(
        container,
        profiles,
        packed_vectors,
        batch.profile.id,
        packed_id,
        body_ids.clone(),
    )?;
    Ok(body_ids.len())
}
