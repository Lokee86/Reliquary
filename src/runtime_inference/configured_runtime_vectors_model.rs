use crate::{CompatibilityProfileId, VectorGenerationId, VectorNormalization};

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
