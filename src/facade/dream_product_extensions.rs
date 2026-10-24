use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, MemoryError, MemoryId,
    Phylactery,
};
impl Cva {
    pub fn dream_memory_context(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        self.native_dream_memory_context(id)
    }
    pub fn dream_candidates(
        &mut self,
        profile: CompatibilityProfileId,
        id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        self.native_dream_candidates(profile, id, config)
    }
    pub fn dream_temporal_analysis(
        &mut self,
        id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        self.native_dream_temporal_analysis(id)
    }
    pub fn publish_dream_pair(
        &mut self,
        classification: &DreamPairClassification,
        verification: Option<&DreamPairVerification>,
        policy: DreamVerificationPolicy,
        expected_graph_version: u64,
    ) -> Result<DreamPublicationOutcome, DreamPublicationError> {
        self.native_publish_dream_pair(classification, verification, policy, expected_graph_version)
    }
    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.native_dream_was_processed(id)
    }
}

impl Phylactery {
    pub(crate) fn dream_eligible_epoch(
        &mut self,
        id: MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, MemoryError> {
        self.native_dream_eligible_epoch(id, now_ns)
    }
    pub(crate) fn mark_dream_processed(
        &mut self,
        id: MemoryId,
        epoch: u64,
        processed_at_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.native_mark_dream_processed(id, epoch, processed_at_ns)
    }
    pub(crate) fn dream_was_processed(&self, id: MemoryId) -> bool {
        self.native_dream_was_processed(id)
    }
}
