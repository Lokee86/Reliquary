use crate::{
    CompatibilityProfileId, Cva, MemoryBodyId, MemoryVectorError, MemoryVectorId, MemoryVectorInfo,
    MemoryVectorLocation, MemoryVectorSet, MemoryVectorStats,
};

impl Cva {
    pub fn memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.public_memory_vectors(id)
    }

    pub fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.runtime_memory_vector_location(compatibility_profile_id, memory_body_id)
    }

    pub fn memory_vector_infos(&self) -> Vec<MemoryVectorInfo> {
        self.public_memory_vector_infos()
    }

    pub fn memory_vector_stats(&self) -> MemoryVectorStats {
        self.public_memory_vector_stats()
    }
}
