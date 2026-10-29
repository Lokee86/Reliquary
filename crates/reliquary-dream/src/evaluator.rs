use crate::{
    DreamCandidateContexts, DreamClassificationError, DreamClassifier, DreamContext,
    DreamPairClassification, DreamPairVerification, DreamVerificationError,
    DreamVerificationPolicy, DreamVerifier, GeneralEndpoint,
};
use std::thread;
const DREAM_INFERENCE_RETRY_LIMIT: usize = 2;
#[derive(Debug)]
pub enum DreamEvaluationError {
    Classification(DreamClassificationError),
    Verification(DreamVerificationError),
}
impl DreamEvaluationError {
    pub fn is_backpressure(&self) -> bool {
        match self {
            Self::Classification(e) => e.is_backpressure(),
            Self::Verification(e) => e.is_backpressure(),
        }
    }
}
impl From<DreamClassificationError> for DreamEvaluationError {
    fn from(e: DreamClassificationError) -> Self {
        Self::Classification(e)
    }
}
impl From<DreamVerificationError> for DreamEvaluationError {
    fn from(e: DreamVerificationError) -> Self {
        Self::Verification(e)
    }
}
pub struct DreamPairEvaluator<C, V> {
    classifier: DreamClassifier<C>,
    verifier: DreamVerifier<V>,
}
impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamPairEvaluator<C, V> {
    pub fn new(c: C, v: V) -> Self {
        Self {
            classifier: DreamClassifier::new(c),
            verifier: DreamVerifier::new(v),
        }
    }
    pub fn with_classifier_system_prompt(c: C, v: V, prompt: impl Into<String>) -> Self {
        Self {
            classifier: DreamClassifier::with_system_prompt(c, prompt),
            verifier: DreamVerifier::new(v),
        }
    }
    pub fn classifier_model(&self) -> &str {
        self.classifier.model()
    }
    pub fn verifier_model(&self) -> &str {
        self.verifier.model()
    }
    pub fn evaluate_candidates<S: DreamCandidateContexts + Sync>(
        &self,
        candidates: &S,
        verification_policy: DreamVerificationPolicy,
        pair_concurrency: usize,
    ) -> Result<Vec<(DreamPairClassification, Option<DreamPairVerification>)>, DreamEvaluationError>
    where
        S::Context: Sync,
    {
        let concurrency = pair_concurrency.max(1);
        let mut evaluated = Vec::with_capacity(candidates.candidates().len());
        let contexts = candidates.candidates();
        for chunk in contexts.chunks(concurrency) {
            let chunk_results = thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|candidate| {
                        scope.spawn(|| {
                            self.evaluate_pair_with_retries(
                                verification_policy,
                                candidates.source(),
                                candidate,
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
                    .collect::<Result<Vec<_>, DreamEvaluationError>>()
            })?;
            evaluated.extend(chunk_results);
        }
        Ok(evaluated)
    }

    pub fn evaluate_pair_with_retries<T: DreamContext>(
        &self,
        verification_policy: DreamVerificationPolicy,
        source: &T,
        candidate: &T,
    ) -> Result<(DreamPairClassification, Option<DreamPairVerification>), DreamEvaluationError>
    {
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

    pub fn evaluate_pair<T: DreamContext>(
        &self,
        verification_policy: DreamVerificationPolicy,
        source: &T,
        candidate: &T,
    ) -> Result<(DreamPairClassification, Option<DreamPairVerification>), DreamEvaluationError>
    {
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
