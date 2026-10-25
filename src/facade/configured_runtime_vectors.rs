use super::{
    ArchiveVectorBuildReport, ConfiguredRuntime, ConfiguredRuntimeError, EmbeddingProbeReport,
    operation,
};
use crate::Cva;
use std::path::Path;

impl ConfiguredRuntime {
    pub fn probe_embedding(
        &self,
        text: &str,
    ) -> Result<EmbeddingProbeReport, ConfiguredRuntimeError> {
        let (vectors, dimensions, normalization) =
            super::provider_operations::probe_embedding(&self.switchboard, text)
                .map_err(operation)?;
        Ok(EmbeddingProbeReport {
            vectors,
            dimensions,
            normalization,
        })
    }

    pub fn build_archive_vectors(
        &self,
        rel_path: &Path,
        batch_size: usize,
        concurrency: usize,
    ) -> Result<ArchiveVectorBuildReport, ConfiguredRuntimeError> {
        let endpoint = super::provider_operations::embedding_endpoint(
            &self.switchboard,
            batch_size,
            concurrency,
        )
        .map_err(operation)?;
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
}
