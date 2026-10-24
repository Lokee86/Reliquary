//! Canonical Dream invocation and accepted owner-prefix access.
impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_population_at_prefix(
        &mut self,
        end: u64,
        cut: u64,
        source: crate::MemoryId,
    ) -> Result<std::collections::BTreeSet<crate::MemoryId>, crate::CvaError> {
        self.owner.dream_population_at_prefix(end, cut, source)
    }
}

use crate::dream_owner_candidates::{
    dream_candidates as candidates_from_parts, dream_memory_context as context_from_parts,
};
use crate::dream_owner_candidates::{
    dream_candidates as dream_candidates_from_parts,
    dream_memory_context as dream_memory_context_from_parts,
};
use crate::dream_publication_execution::publish_dream_pair as publish_from_parts;
use crate::dream_temporal_policy::analyze_memory_temporal;
use crate::memory_source_time::memory_source_timestamp_ns;
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, Memory, MemoryError,
    MemoryId,
};

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_memory_context(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let archive = &self.archive;
        context_from_parts(&facts.memories, facts.graph, id, &|memory: &Memory| {
            reliquary_memory_source_timestamp_ns(archive, memory)
        })
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_candidates(
        &mut self,
        profile: CompatibilityProfileId,
        id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let archive = &self.archive;
        candidates_from_parts(
            &facts.memories,
            facts.graph,
            facts.pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(facts.vectors),
            profile,
            id,
            config,
            &|memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory),
        )
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_temporal_analysis(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        let memory = self.owner.memories.memory(&mut self.owner.container, id)?;
        let body = self.memories.current_body_id(id)?;
        Ok(analyze_memory_temporal(
            &memory,
            body,
            reliquary_memory_source_timestamp_ns(&self.archive, &memory),
        ))
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        let archive = &self.owner.archive;
        publish_from_parts(
            crate::owner_lifecycle::DreamGraphPublication::new(
                &mut self.owner.container,
                &self.owner.memories,
                &mut self.owner.graph,
            ),
            &mut self.duplicate_index,
            classification,
            verification,
            policy,
            expected_graph_version,
            &|memory: &Memory| reliquary_memory_source_timestamp_ns(archive, memory),
        )
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_was_processed(&self, id: MemoryId) -> bool {
        self.owner
            .dream_processing_state_native(id)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        let memory = self.memory(id)?;
        crate::dream_cooldown::eligible_dream_epoch(
            &memory,
            memory_source_timestamp_ns(&memory),
            self.owner.dream_processing_state_native(id),
            now_ns,
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_processed_native(
            id,
            crate::dream_cooldown::dream_processing_state(epoch, processed_at_ns),
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_was_processed(&self, id: MemoryId) -> bool {
        self.owner
            .dream_processing_state_native(id)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_memory_context(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let source_time = |memory: &Memory| memory_source_timestamp_ns(memory);
        dream_memory_context_from_parts(&facts.memories, facts.graph, memory_id, &source_time)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_candidates(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        let facts = self.owner.dream_operation_facts();
        let source_time = |memory: &Memory| memory_source_timestamp_ns(memory);
        dream_candidates_from_parts(
            &facts.memories,
            facts.graph,
            facts.pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(facts.vectors),
            compatibility_profile_id,
            source_id,
            config,
            &source_time,
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_temporal_analysis(
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

impl crate::RelExecutionOwner {
    pub(crate) fn native_mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_pair_native(left, right)
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.owner.dream_pair_records_native()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.owner.mark_dream_pair_native(left, right)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.owner.dream_pair_records_native()
    }
}
