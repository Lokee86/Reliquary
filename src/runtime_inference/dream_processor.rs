use crate::{
    DreamCandidateSet, DreamClassificationError, DreamClassifier, DreamMemoryContext,
    DreamPairClassification, DreamPairVerification, DreamProcessError, DreamVerificationError,
    DreamVerificationPolicy, DreamVerifier, GeneralEndpoint,
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
