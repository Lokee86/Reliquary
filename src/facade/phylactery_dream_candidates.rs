use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, MemoryError, MemoryId,
    Phylactery,
};
impl Phylactery {
    pub fn dream_memory_context(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamMemoryContext, DreamCandidateError> {
        self.native_dream_memory_context(memory_id)
    }

    pub fn dream_candidates(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        config: DreamCandidateConfig,
    ) -> Result<DreamCandidateSet, DreamCandidateError> {
        self.native_dream_candidates(compatibility_profile_id, source_id, config)
    }

    pub fn dream_temporal_analysis(
        &mut self,
        memory_id: MemoryId,
    ) -> Result<DreamTemporalAnalysis, MemoryError> {
        self.native_dream_temporal_analysis(memory_id)
    }
}
