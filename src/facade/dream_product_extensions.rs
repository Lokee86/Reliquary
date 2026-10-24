use crate::dream_owner_candidates::{
    dream_candidates as candidates_from_parts, dream_memory_context as context_from_parts,
};
use crate::dream_owner_publisher::publish_dream_pair as publish_from_parts;
use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, Memory, MemoryError,
    MemoryId, Phylactery,
};

impl Cva {
    pub fn dream_memory_context(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let archive = &self.archive;
        context_from_parts(
            &mut self.container,
            &self.memories,
            &self.graph,
            id,
            &|memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory),
        )
    }
    pub fn dream_candidates(
        &mut self,
        profile: CompatibilityProfileId,
        id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let archive = &self.archive;
        candidates_from_parts(
            &mut self.container,
            &self.memories,
            &self.graph,
            &self.dream_pairs,
            &self.memory_vectors,
            &self.packed_vectors,
            profile,
            id,
            config,
            &|memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory),
        )
    }
    pub fn dream_temporal_analysis(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self.memories.memory(&mut self.container, id)?;
        let body = self.memories.current_body_id(id)?;
        Ok(analyze_memory_temporal(
            &memory,
            body,
            reliquary_memory_source_timestamp_ns(&self.archive, &memory),
        ))
    }
    pub fn publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        let archive = &self.archive;
        publish_from_parts(
            &mut self.container,
            &self.memories,
            &mut self.graph,
            &mut self.duplicate_index,
            classification,
            verification,
            policy,
            expected_graph_version,
            &|memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory),
        )
    }
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        crate::dream_cooldown::eligible_dream_epoch(
            &memory,
            reliquary_memory_source_timestamp_ns(&self.archive, &memory),
            self.processing_epochs
                .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE),
            now_ns,
        )
    }
    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(id) {
            return Err(MemoryError::MissingMemory);
        }
        self.processing_epochs.put(
            &mut self.container,
            id,
            crate::dream_cooldown::DREAM_PROCESSING_LANE,
            crate::dream_cooldown::dream_processing_state(epoch, processed_at_ns),
        )
    }
    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.processing_epochs
            .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl Phylactery {
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        crate::dream_cooldown::eligible_dream_epoch(
            &memory,
            memory_source_timestamp_ns(&memory),
            self.processing_epochs
                .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE),
            now_ns,
        )
    }
    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(id) {
            return Err(MemoryError::MissingMemory);
        }
        self.processing_epochs.put(
            &mut self.container,
            id,
            crate::dream_cooldown::DREAM_PROCESSING_LANE,
            crate::dream_cooldown::dream_processing_state(epoch, processed_at_ns),
        )
    }
    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.processing_epochs
            .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}
