use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamMemoryProcessOutcome,
    DreamProcessError, DreamProcessResult, DreamProcessedPair, DreamProcessor,
    DreamVerificationPolicy, GeneralEndpoint, MemoryId, Phylactery,
};

impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamProcessor<C, V> {
    /// Process deterministic frontiers with bounded pair inference, publishing in source order.
    /// Failed classification or verification leaves that Memory unchanged.
    pub fn process_memories_with_concurrency(
        &self,
        cva: &mut Cva,
        compatibility_profile_id: CompatibilityProfileId,
        source_ids: &[MemoryId],
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        frontier_size: usize,
        inference_concurrency: usize,
    ) -> Result<Vec<DreamMemoryProcessOutcome>, DreamProcessError> {
        self.runtime_process_memories_with_concurrency(
            cva,
            compatibility_profile_id,
            source_ids,
            candidate_config,
            verification_policy,
            frontier_size,
            inference_concurrency,
        )
    }

    /// PHY equivalent of [`Self::process_memories_with_concurrency`].
    pub fn process_phylactery_memories_with_concurrency(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        source_ids: &[MemoryId],
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        frontier_size: usize,
        inference_concurrency: usize,
    ) -> Result<Vec<DreamMemoryProcessOutcome>, DreamProcessError> {
        self.runtime_process_phylactery_memories_with_concurrency(
            phylactery,
            compatibility_profile_id,
            source_ids,
            candidate_config,
            verification_policy,
            frontier_size,
            inference_concurrency,
        )
    }
}
