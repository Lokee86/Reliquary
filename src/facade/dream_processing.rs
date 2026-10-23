//! Supported offline Dream processing composition; mounted workers use the pure evaluator.
use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamProcessError, DreamProcessResult,
    DreamProcessedPair, DreamProcessor, DreamVerificationPolicy, GeneralEndpoint, MemoryId,
    Phylactery,
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
        self.process_memory_with_pair_concurrency(
            cva,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            1,
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
        let candidates =
            cva.dream_candidates(compatibility_profile_id, source_id, candidate_config)?;
        let candidate_count = candidates.candidates.len();
        let evaluated =
            self.evaluate_candidates(&candidates, verification_policy, pair_concurrency)?;
        crate::dream_freshness::begin_initial_pass(
            cva,
            source_id,
            candidates
                .candidates
                .iter()
                .map(|candidate| candidate.context.memory.id),
        )
        .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
        let mut pairs = Vec::with_capacity(candidate_count);
        for (classification, verification) in evaluated {
            let publication = cva.publish_dream_pair(
                &classification,
                verification.as_ref(),
                verification_policy,
                cva.graph_version(),
            )?;
            crate::dream_freshness::record_publication_roots(cva, source_id, &publication)
                .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
            cva.mark_dream_pair_evaluated(classification.a, classification.b)?;
            pairs.push(DreamProcessedPair {
                classification,
                verification,
                publication,
            });
        }
        if let Some(pass) = cva.pending_initial_dream_pass(source_id) {
            cva.accept_initial_dream_settlement(&pass.pass_id, cva.rel_turn_count())
                .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
        }
        let lifecycle = cva.reconcile_dream_lifecycle(source_id)?;
        let active_admission = !lifecycle.source.archived
            && matches!(
                lifecycle.source.lifecycle_state.as_str(),
                "knowledge" | "canonical"
            );
        crate::dream_freshness::settle_initial_pass(
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

    pub fn process_phylactery_memory(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        source_id: MemoryId,
        candidate_config: DreamCandidateConfig,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamProcessResult, DreamProcessError> {
        self.process_phylactery_memory_with_pair_concurrency(
            phylactery,
            compatibility_profile_id,
            source_id,
            candidate_config,
            verification_policy,
            1,
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
        let candidates =
            phylactery.dream_candidates(compatibility_profile_id, source_id, candidate_config)?;
        let candidate_count = candidates.candidates.len();
        let evaluated =
            self.evaluate_candidates(&candidates, verification_policy, pair_concurrency)?;
        let mut pairs = Vec::with_capacity(candidate_count);
        for (classification, verification) in evaluated {
            let publication = phylactery.publish_dream_pair(
                &classification,
                verification.as_ref(),
                verification_policy,
                phylactery.graph_version(),
            )?;
            phylactery.mark_dream_pair_evaluated(classification.a, classification.b)?;
            pairs.push(DreamProcessedPair {
                classification,
                verification,
                publication,
            });
        }
        let lifecycle = phylactery.reconcile_dream_lifecycle(source_id)?;
        Ok(DreamProcessResult {
            source: lifecycle.source.clone(),
            candidate_count,
            pairs,
            lifecycle,
        })
    }
}
