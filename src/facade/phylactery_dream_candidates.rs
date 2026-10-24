use crate::dream_owner_candidates::{
    dream_candidates as dream_candidates_from_parts,
    dream_memory_context as dream_memory_context_from_parts,
};
use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamTemporalAnalysis, Memory, MemoryError, MemoryId, Phylactery,
};

impl Phylactery {
    pub fn dream_memory_context(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let source_time = |memory: &Memory| memory_source_timestamp_ns(memory);
        dream_memory_context_from_parts(
            &crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
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
        let source_time = |memory: &Memory| memory_source_timestamp_ns(memory);
        dream_candidates_from_parts(
            &crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
            &self.graph,
            &self.dream_pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(
                crate::memory_vector_store::MemoryVectorReadView::new(
                    &self.memory_vectors,
                    &self.packed_vectors,
                    &self.container,
                ),
            ),
            compatibility_profile_id,
            source_id,
            config,
            &source_time,
        )
    }

    pub fn dream_temporal_analysis(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self
            .owner
            .memories
            .memory(&mut self.owner.container, memory_id)?;
        let body_id = self.memories.current_body_id(memory_id)?;
        Ok(analyze_memory_temporal(
            &memory,
            body_id,
            memory_source_timestamp_ns(&memory),
        ))
    }
}
