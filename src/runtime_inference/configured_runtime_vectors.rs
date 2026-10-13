use super::{ConfiguredRuntime, ConfiguredRuntimeError, operation};
use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode, OpenAiReadyEmbeddingEndpoint,
    VectorGenerationId, VectorNormalization,
};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct EmbeddingProbeReport {
    pub vectors: usize,
    pub dimensions: usize,
    pub normalization: VectorNormalization,
}

#[derive(Clone, Copy, Debug)]
pub struct ArchiveVectorBuildReport {
    pub profile_id: CompatibilityProfileId,
    pub generation_id: VectorGenerationId,
    pub source_archive_version: u64,
    pub vector_version: u64,
}

impl ConfiguredRuntime {
    pub fn probe_embedding(
        &self,
        text: &str,
    ) -> Result<EmbeddingProbeReport, ConfiguredRuntimeError> {
        let endpoint =
            OpenAiReadyEmbeddingEndpoint::from_switchboard(&self.switchboard).map_err(operation)?;
        let vectors = endpoint
            .embed(EmbeddingMode::Query, &[text.to_owned()])
            .map_err(operation)?;
        Ok(EmbeddingProbeReport {
            vectors: vectors.len(),
            dimensions: vectors.first().map(Vec::len).unwrap_or(0),
            normalization: endpoint.normalization(),
        })
    }

    pub fn build_archive_vectors(
        &self,
        rel_path: &Path,
        batch_size: usize,
        concurrency: usize,
    ) -> Result<ArchiveVectorBuildReport, ConfiguredRuntimeError> {
        let endpoint = self.embedding_endpoint(batch_size, concurrency)?;
        let mut rel = Cva::open(rel_path).map_err(operation)?;
        let profile = rel
            .establish_compatibility_profile(&endpoint)
            .map_err(operation)?;
        let generation = rel
            .build_archive_vector_generation(profile.id, &endpoint)
            .map_err(operation)?;
        rel.sync().map_err(operation)?;
        Ok(ArchiveVectorBuildReport {
            profile_id: profile.id,
            generation_id: generation.id,
            source_archive_version: generation.source_archive_version,
            vector_version: generation.vector_version,
        })
    }

    pub(super) fn embedding_endpoint(
        &self,
        batch_size: usize,
        concurrency: usize,
    ) -> Result<OpenAiReadyEmbeddingEndpoint, ConfiguredRuntimeError> {
        OpenAiReadyEmbeddingEndpoint::from_switchboard(&self.switchboard)
            .map_err(operation)?
            .with_batching(batch_size, concurrency)
            .map_err(operation)
    }
}
