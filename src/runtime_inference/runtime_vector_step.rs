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

pub(crate) fn select_body_ids(
    profile_id: CompatibilityProfileId,
    current: &[MemoryBodyId],
    missing: impl Fn(CompatibilityProfileId, &[MemoryBodyId]) -> Vec<MemoryBodyId>,
) -> Vec<MemoryBodyId> {
    if current.is_empty() {
        Vec::new()
    } else {
        missing(profile_id, current)
    }
}

pub(crate) fn pending_rows(
    profile_id: CompatibilityProfileId,
    body_ids: Vec<MemoryBodyId>,
    vectors: Vec<Vec<f32>>,
    mut has_location: impl FnMut(CompatibilityProfileId, MemoryBodyId) -> bool,
) -> (Vec<MemoryBodyId>, Vec<Vec<f32>>) {
    let mut pending_ids = Vec::with_capacity(body_ids.len());
    let mut pending_vectors = Vec::with_capacity(vectors.len());
    for (body_id, vector) in body_ids.into_iter().zip(vectors) {
        if !has_location(profile_id, body_id) {
            pending_ids.push(body_id);
            pending_vectors.push(vector);
        }
    }
    (pending_ids, pending_vectors)
}

pub(crate) fn validate_body_presence(
    body_ids: &[MemoryBodyId],
    mut contains: impl FnMut(MemoryBodyId) -> bool,
) -> Result<(), MemoryVectorError> {
    if body_ids.iter().any(|id| !contains(*id)) {
        Err(MemoryVectorError::MissingMemoryBody)
    } else {
        Ok(())
    }
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
    let (body_ids, pending_vectors) = pending_rows(
        batch.profile.id,
        batch.body_ids,
        vectors,
        |profile, body| memory_vectors.location(profile, body).is_some(),
    );
    if body_ids.is_empty() {
        return Ok(None);
    }
    validate_body_presence(&body_ids, |id| memories.contains_body(id))?;
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
