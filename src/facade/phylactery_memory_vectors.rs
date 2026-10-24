use crate::{
    CompatibilityProfileId, EmbeddingEndpoint, MemoryBodyId, MemoryVectorBuildResult,
    MemoryVectorError, MemoryVectorId, MemoryVectorInfo, MemoryVectorSet, MemoryVectorStats,
    PackedVectorId, Phylactery,
};

impl Phylactery {
    pub fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.public_put_memory_vectors(compatibility_profile_id, packed_vector_id, memory_body_ids)
    }

    pub fn memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.public_memory_vectors(id)
    }

    pub fn memory_vector_infos(&self) -> Vec<MemoryVectorInfo> {
        self.public_memory_vector_infos()
    }

    pub fn memory_vector_stats(&self) -> MemoryVectorStats {
        self.public_memory_vector_stats()
    }

    pub fn build_missing_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<MemoryVectorBuildResult, MemoryVectorError> {
        self.public_build_missing_memory_vectors(compatibility_profile_id, endpoint)
    }
}
