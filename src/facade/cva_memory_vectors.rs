use crate::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
use crate::{
    CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode, MemoryBodyId,
    MemoryVectorBuildResult, MemoryVectorError, MemoryVectorId, MemoryVectorInfo,
    MemoryVectorLocation, MemoryVectorSet, MemoryVectorStats, PackedVectorId, PackedVectors,
    ScalarType, VectorSchema,
};

impl Cva {
    pub fn memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.owner.memory_vectors.get(&mut self.owner.container, id)
    }

    pub fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.runtime_memory_vector_location(compatibility_profile_id, memory_body_id)
    }

    pub fn memory_vector_infos(&self) -> Vec<MemoryVectorInfo> {
        self.memory_vectors.infos()
    }

    pub fn memory_vector_stats(&self) -> MemoryVectorStats {
        self.memory_vectors.stats()
    }
}
