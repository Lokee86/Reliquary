use crate::dream_owner_candidates::{
    dream_candidates as dream_candidates_from_parts,
    dream_memory_context as dream_memory_context_from_parts,
};
use crate::memory_source_time::reliquary_memory_source_timestamp_ns;
use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, Memory, MemoryId,
};

impl Cva {
    pub fn dream_memory_context(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let archive = &self.archive;
        let source_time = |memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory);
        dream_memory_context_from_parts(
            &mut self.container,
            &self.memories,
            &self.graph,
            memory_id,
            &source_time,
        )
    }

    pub fn dream_candidates(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let archive = &self.archive;
        let source_time = |memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory);
        dream_candidates_from_parts(
            &mut self.container,
            &self.memories,
            &self.graph,
            &self.dream_pairs,
            &self.memory_vectors,
            &self.packed_vectors,
            compatibility_profile_id,
            source_id,
            config,
            &source_time,
        )
    }
}
