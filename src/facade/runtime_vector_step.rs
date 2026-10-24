use crate::runtime_vector_step_owner::RuntimeMemoryVectorBatch;
use crate::{
    CompatibilityProfile, CompatibilityProfileError, CompatibilityProfileId, Cva,
    MemoryVectorError, Phylactery,
};

impl Cva {
    pub(crate) fn accept_runtime_compatibility_profile(
        &mut self,
        candidate: CompatibilityProfile,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        crate::runtime_vector_step_owner::accept_profile(
            &mut self.container,
            &mut self.compatibility_profiles,
            candidate,
        )
    }

    pub(crate) fn prepare_runtime_memory_vector_batch(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
        crate::runtime_vector_step_owner::prepare_batch(
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
        crate::runtime_vector_step_owner::commit_batch(
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
        crate::runtime_vector_step_owner::accept_profile(
            &mut self.container,
            &mut self.compatibility_profiles,
            candidate,
        )
    }

    pub(crate) fn prepare_runtime_memory_vector_batch(
        &mut self,
        profile_id: CompatibilityProfileId,
    ) -> Result<Option<RuntimeMemoryVectorBatch>, MemoryVectorError> {
        crate::runtime_vector_step_owner::prepare_batch(
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
        crate::runtime_vector_step_owner::commit_batch(
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
