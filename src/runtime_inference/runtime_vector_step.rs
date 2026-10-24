use crate::compatibility_profile_probe::validate_embedding_batch;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::{MemoryVectorPublicationPlan, MemoryVectorStore};
use crate::{
    CompatibilityProfile, EmbeddingEndpoint, EmbeddingMode, MemoryBodyId, MemoryVectorError,
    PackedVectors, ScalarType, VectorSchema,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExternalEmbeddingError {
    Unavailable,
    InvalidProfile,
}

/// Vector-owned provider protocol consumed by External Sources. The provider
/// implementation stays in this component, keeping provider callbacks out of
/// the source-policy owner.
pub(crate) trait ExternalEmbeddingService {
    fn verify_profile(&self, profile: &CompatibilityProfile) -> Result<(), ExternalEmbeddingError>;
    fn embed_documents(
        &self,
        profile: &CompatibilityProfile,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, ExternalEmbeddingError>;
    fn embed_query(
        &self,
        profile: &CompatibilityProfile,
        query: &str,
    ) -> Result<Vec<Vec<f32>>, ExternalEmbeddingError>;
}

impl<E: EmbeddingEndpoint> ExternalEmbeddingService for E {
    fn verify_profile(&self, profile: &CompatibilityProfile) -> Result<(), ExternalEmbeddingError> {
        if profile.dimensions == 0 || profile.dimensions > 4096 {
            return Err(ExternalEmbeddingError::InvalidProfile);
        }
        let report = crate::compatibility_profile_probe::verify_endpoint(profile, self)
            .map_err(|_| ExternalEmbeddingError::Unavailable)?;
        report
            .compatible
            .then_some(())
            .ok_or(ExternalEmbeddingError::InvalidProfile)
    }

    fn embed_documents(
        &self,
        _profile: &CompatibilityProfile,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, ExternalEmbeddingError> {
        self.embed(EmbeddingMode::Document, inputs)
            .map_err(|_| ExternalEmbeddingError::Unavailable)
    }

    fn embed_query(
        &self,
        _profile: &CompatibilityProfile,
        query: &str,
    ) -> Result<Vec<Vec<f32>>, ExternalEmbeddingError> {
        self.embed(EmbeddingMode::Query, &[query.to_owned()])
            .map_err(|_| ExternalEmbeddingError::Unavailable)
    }
}

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
