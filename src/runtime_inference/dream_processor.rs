use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateSet, DreamClassificationError,
    DreamClassifier, DreamMemoryContext, DreamPairClassification, DreamPairVerification,
    DreamProcessError, DreamProcessResult, DreamProcessedPair, DreamVerificationError,
    DreamVerificationPolicy, DreamVerifier, GeneralEndpoint, MemoryId, Phylactery,
};
use std::thread;

const DREAM_INFERENCE_RETRY_LIMIT: usize = 2;

pub struct DreamProcessor<C, V> {
    classifier: DreamClassifier<C>,
    verifier: DreamVerifier<V>,
}

impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamProcessor<C, V> {
    pub fn new(classifier_endpoint: C, verifier_endpoint: V) -> Self {
        Self {
            classifier: DreamClassifier::new(classifier_endpoint),
            verifier: DreamVerifier::new(verifier_endpoint),
        }
    }

    pub fn with_classifier_system_prompt(
        classifier_endpoint: C,
        verifier_endpoint: V,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            classifier: DreamClassifier::with_system_prompt(classifier_endpoint, system_prompt),
            verifier: DreamVerifier::new(verifier_endpoint),
        }
    }

    pub fn classifier_model(&self) -> &str {
        self.classifier.model()
    }

    pub fn verifier_model(&self) -> &str {
        self.verifier.model()
    }

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

    pub(crate) fn evaluate_candidates(
        &self,
        candidates: &DreamCandidateSet,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<Vec<(DreamPairClassification, Option<DreamPairVerification>)>, DreamProcessError>
    {
        let concurrency = pair_concurrency.max(1);
        let mut evaluated = Vec::with_capacity(candidates.candidates.len());
        for chunk in candidates.candidates.chunks(concurrency) {
            let chunk_results = thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|candidate| {
                        scope.spawn(|| {
                            self.evaluate_pair_with_retries(
                                verification_policy,
                                &candidates.source,
                                &candidate.context,
                            )
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| match handle.join() {
                        Ok(result) => result,
                        Err(payload) => std::panic::resume_unwind(payload),
                    })
                    .collect::<Result<Vec<_>, DreamProcessError>>()
            })?;
            evaluated.extend(chunk_results);
        }
        Ok(evaluated)
    }

    pub(crate) fn evaluate_pair_with_retries(
        &self,
        verification_policy: DreamVerificationPolicy,
        source: &DreamMemoryContext,
        candidate: &DreamMemoryContext,
    ) -> Result<(DreamPairClassification, Option<DreamPairVerification>), DreamProcessError> {
        for attempt in 0..=DREAM_INFERENCE_RETRY_LIMIT {
            match self.evaluate_pair(verification_policy, source, candidate) {
                Ok(result) => return Ok(result),
                Err(error) if error.is_backpressure() => return Err(error),
                Err(error) if attempt == DREAM_INFERENCE_RETRY_LIMIT => return Err(error),
                Err(_) => continue,
            }
        }
        unreachable!("Dream inference retry loop always returns")
    }

    pub(crate) fn evaluate_pair(
        &self,
        verification_policy: DreamVerificationPolicy,
        source: &DreamMemoryContext,
        candidate: &DreamMemoryContext,
    ) -> Result<(DreamPairClassification, Option<DreamPairVerification>), DreamProcessError> {
        let classification = self
            .classifier
            .classify_pair(source, candidate)
            .map_err(DreamClassificationError::from)?;
        let verification = self
            .verifier
            .verify_if_required(verification_policy, &classification, source, candidate)
            .map_err(DreamVerificationError::from)?;
        Ok((classification, verification))
    }
}
