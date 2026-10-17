use crate::compatibility_profile_probe::validate_embedding_batch;
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId, Cva, MemoryBodyId,
    MemoryVectorError, PackedVectors, Phylactery, ScalarType, VectorSchema,
};

pub(crate) struct RuntimeMemoryVectorBatch {
    pub(crate) profile: CompatibilityProfile,
    pub(crate) body_ids: Vec<MemoryBodyId>,
    pub(crate) texts: Vec<String>,
}

impl Cva {
    pub(crate) fn accept_runtime_compatibility_profile(
        &mut self,
        candidate: CompatibilityProfile,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        accept_profile(
            &mut self.container,
            &mut self.compatibility_profiles,
            candidate,
        )
    }

    pub(crate) fn prepare_runtime_memory_vector_batch(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
        prepare_batch(
            &mut self.container,
            &self.memories,
            &self.compatibility_profiles,
            &self.memory_vectors,
            profile_id,
        )
    }

    pub(crate) fn commit_runtime_memory_vector_batch(
        &mut self,
        batch: RuntimeMemoryVectorBatch,
        vectors: Vec<Vec<f32>>,
    ) -> Result<usize, MemoryVectorError> {
        commit_batch(
            &mut self.container,
            &self.memories,
            &self.compatibility_profiles,
            &mut self.packed_vectors,
            &mut self.memory_vectors,
            batch,
            vectors,
        )
    }
}

impl Phylactery {
    pub(crate) fn accept_runtime_compatibility_profile(
        &mut self,
        candidate: CompatibilityProfile,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        accept_profile(
            &mut self.container,
            &mut self.compatibility_profiles,
            candidate,
        )
    }

    pub(crate) fn prepare_runtime_memory_vector_batch(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
        prepare_batch(
            &mut self.container,
            &self.memories,
            &self.compatibility_profiles,
            &self.memory_vectors,
            profile_id,
        )
    }

    pub(crate) fn commit_runtime_memory_vector_batch(
        &mut self,
        batch: RuntimeMemoryVectorBatch,
        vectors: Vec<Vec<f32>>,
    ) -> Result<usize, MemoryVectorError> {
        commit_batch(
            &mut self.container,
            &self.memories,
            &self.compatibility_profiles,
            &mut self.packed_vectors,
            &mut self.memory_vectors,
            batch,
            vectors,
        )
    }
}

fn accept_profile(
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

fn prepare_batch(
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

fn commit_batch(
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
        memories,
        profiles,
        packed_vectors,
        batch.profile.id,
        packed_vector_id,
        body_ids.clone(),
    )?;
    Ok(body_ids.len())
}
