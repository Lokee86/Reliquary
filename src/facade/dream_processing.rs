//! Supported offline Dream processing composition; mounted workers use the pure evaluator.
use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamProcessError, DreamProcessResult,
    DreamProcessor, DreamVerificationPolicy, GeneralEndpoint, MemoryId, Phylactery,
};

impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamProcessor<C, V> {
    pub fn process_memory(
        &self,
        cva: &mut Cva,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_memory(
            cva,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
        )
    }

    pub fn process_memory_with_pair_concurrency(
        &self,
        cva: &mut Cva,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_memory_with_pair_concurrency(
            cva,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            pair_concurrency,
        )
    }

    pub fn process_phylactery_memory(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_phylactery_memory(
            phylactery,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
        )
    }

    pub fn process_phylactery_memory_with_pair_concurrency(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_phylactery_memory_with_pair_concurrency(
            phylactery,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            pair_concurrency,
        )
    }
}
