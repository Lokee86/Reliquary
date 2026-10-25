//! Supported offline Dream processing composition; mounted workers use the pure evaluator.
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamProcessError, DreamProcessResult,
    DreamProcessedPair, DreamProcessor, DreamVerificationPolicy, GeneralEndpoint, MemoryId,
    RelExecutionOwner, phy_execution_owner::PhyExecutionOwner,
};

impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamProcessor<C, V> {
    pub(crate) fn runtime_process_memory(
        &self,
        cva: &mut RelExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_memory_with_pair_concurrency(
            cva,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            1,
        )
    }

    pub(crate) fn runtime_process_memory_with_pair_concurrency(
        &self,
        cva: &mut RelExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        let candidates =
            cva.native_dream_candidates(compatibility_profile_id, source_id, candidate_config)?;
        let candidate_count = candidates.candidates.len();
        let evaluated =
            self.evaluate_candidates(&candidates, verification_policy, pair_concurrency)?;
        crate::freshness_operations_runtime::begin_initial_dream_pass(cva, source_id)
            .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
        let mut pairs = Vec::with_capacity(candidate_count);
        for (classification, verification) in evaluated {
            let publication = cva.native_publish_dream_pair(
                &classification,
                verification.as_ref(),
                verification_policy,
                cva.runtime_graph_version(),
            )?;
            crate::freshness_operations_runtime::record_initial_dream_publication_roots(
                cva,
                source_id,
                &publication,
            )
            .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
            cva.native_mark_dream_pair_evaluated(classification.a, classification.b)?;
            pairs.push(DreamProcessedPair {
                classification,
                verification,
                publication,
            });
        }
        if let Some(pass) = cva.native_pending_initial_dream_pass(source_id) {
            cva.native_accept_initial_dream_settlement(&pass.pass_id, cva.runtime_rel_turn_count())
                .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
        }
        let lifecycle = cva.native_reconcile_dream_lifecycle(source_id)?;
        let active_admission = !lifecycle.source.archived
            && matches!(
                lifecycle.source.lifecycle_state.as_str(),
                "knowledge" | "canonical"
            );
        crate::freshness_operations_runtime::settle_initial_dream_pass(
            cva,
            source_id,
            active_admission,
            crate::freshness::default_worker_count(),
        )
        .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
        Ok(DreamProcessResult {
            source: lifecycle.source.clone(),
            candidate_count,
            pairs,
            lifecycle,
        })
    }

    pub(crate) fn runtime_process_phylactery_memory(
        &self,
        phylactery: &mut PhyExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.runtime_process_phylactery_memory_with_pair_concurrency(
            phylactery,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            1,
        )
    }

    pub(crate) fn runtime_process_phylactery_memory_with_pair_concurrency(
        &self,
        phylactery: &mut PhyExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        let candidates = phylactery.native_dream_candidates(
            compatibility_profile_id,
            source_id,
            candidate_config,
        )?;
        let candidate_count = candidates.candidates.len();
        let evaluated =
            self.evaluate_candidates(&candidates, verification_policy, pair_concurrency)?;
        let mut pairs = Vec::with_capacity(candidate_count);
        for (classification, verification) in evaluated {
            let publication = phylactery.native_publish_dream_pair(
                &classification,
                verification.as_ref(),
                verification_policy,
                phylactery.graph_version(),
            )?;
            phylactery.native_mark_dream_pair_evaluated(classification.a, classification.b)?;
            pairs.push(DreamProcessedPair {
                classification,
                verification,
                publication,
            });
        }
        let lifecycle = phylactery.native_reconcile_dream_lifecycle(source_id)?;
        Ok(DreamProcessResult {
            source: lifecycle.source.clone(),
            candidate_count,
            pairs,
            lifecycle,
        })
    }
}
