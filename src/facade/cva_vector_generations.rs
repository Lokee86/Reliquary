use crate::{
    ArchiveVectorId, CompatibilityProfile, CompatibilityProfileId, Cva, EmbeddingEndpoint,
    VectorGeneration, VectorGenerationError, VectorGenerationId, VectorGenerationStats,
};

impl Cva {
    pub fn build_archive_vector_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_build_archive_vector_generation(compatibility_profile_id, endpoint)
    }

    pub(crate) fn build_archive_vector_generation_verified(
        &mut self,
        profile: CompatibilityProfile,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_build_archive_vector_generation_verified(profile, endpoint)
    }

    pub fn publish_vector_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_publish_vector_generation(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )
    }

    pub(crate) fn publish_vector_generation_without_live_semantic(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_publish_vector_generation_without_live_semantic(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )
    }

    pub fn vector_generation(
        &self,
        id: VectorGenerationId,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_vector_generation(id)
    }

    pub fn current_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.public_current_vector_generation(compatibility_profile_id)
    }

    pub fn vector_generation_at(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        vector_version: u64,
    ) -> Option<VectorGeneration> {
        self.public_vector_generation_at(compatibility_profile_id, vector_version)
    }

    pub fn historical_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.public_historical_vector_generation(compatibility_profile_id)
    }

    pub fn vector_version(&self) -> u64 {
        self.public_vector_version()
    }

    pub fn vector_generation_stats(&self) -> VectorGenerationStats {
        self.public_vector_generation_stats()
    }
}
