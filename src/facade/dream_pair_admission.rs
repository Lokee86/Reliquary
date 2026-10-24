use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateError, DreamCandidateSet,
    DreamMemoryContext, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamTemporalAnalysis, DreamVerificationPolicy, MemoryError, MemoryId,
    Phylactery,
};
impl Cva {
    pub(crate) fn mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.native_mark_dream_pair_evaluated(left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.native_dream_pair_records()
    }
}

impl Phylactery {
    pub(crate) fn mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        self.native_mark_dream_pair_evaluated(left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.native_dream_pair_records()
    }
}
