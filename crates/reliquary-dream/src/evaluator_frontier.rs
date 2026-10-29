use crate::{
    DreamCandidateContexts, DreamEvaluationError, DreamPairClassification, DreamPairEvaluator,
    DreamPairVerification, DreamVerificationPolicy, GeneralEndpoint,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
type PairEvaluation =
    Result<(DreamPairClassification, Option<DreamPairVerification>), DreamEvaluationError>;
impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamPairEvaluator<C, V> {
    pub fn evaluate_frontier<S: DreamCandidateContexts + Sync>(
        &self,
        staged: &[&S],
        verification_policy: DreamVerificationPolicy,
        inference_concurrency: usize,
    ) -> Result<Vec<Vec<PairEvaluation>>, DreamEvaluationError>
    where
        S::Context: Sync,
    {
        let mut work = Vec::new();
        let max_candidates = staged
            .iter()
            .map(|candidates| candidates.candidates().len())
            .max()
            .unwrap_or(0);
        for candidate_index in 0..max_candidates {
            for (memory_index, candidates) in staged.iter().enumerate() {
                if let Some(candidate) = candidates.candidates().get(candidate_index).copied() {
                    work.push((
                        memory_index,
                        candidate_index,
                        candidates.source(),
                        candidate,
                    ));
                }
            }
        }

        let mut evaluated: Vec<Vec<Option<PairEvaluation>>> = staged
            .iter()
            .map(|candidates| {
                std::iter::repeat_with(|| None)
                    .take(candidates.candidates().len())
                    .collect()
            })
            .collect();

        let worker_count = inference_concurrency.max(1).min(work.len());
        if worker_count > 0 {
            let next = AtomicUsize::new(0);
            let stop = AtomicBool::new(false);
            let results = thread::scope(|scope| {
                let handles: Vec<_> = (0..worker_count)
                    .map(|_| {
                        scope.spawn(|| {
                            let mut local = Vec::new();
                            loop {
                                if stop.load(Ordering::Acquire) {
                                    break;
                                }
                                let work_index = next.fetch_add(1, Ordering::Relaxed);
                                let Some(&(memory_index, candidate_index, source, candidate)) =
                                    work.get(work_index)
                                else {
                                    break;
                                };
                                let result = self.evaluate_pair_with_retries(
                                    verification_policy,
                                    source,
                                    candidate,
                                );
                                let backpressure = result
                                    .as_ref()
                                    .is_err_and(DreamEvaluationError::is_backpressure);
                                local.push((memory_index, candidate_index, result));
                                if backpressure {
                                    stop.store(true, Ordering::Release);
                                    break;
                                }
                            }
                            local
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|handle| match handle.join() {
                        Ok(results) => results,
                        Err(payload) => std::panic::resume_unwind(payload),
                    })
                    .collect::<Vec<_>>()
            });
            for (memory_index, candidate_index, result) in results {
                match result {
                    Err(error) if error.is_backpressure() => return Err(error),
                    result => evaluated[memory_index][candidate_index] = Some(result),
                }
            }
        }

        Ok(evaluated
            .into_iter()
            .map(|memory| {
                memory
                    .into_iter()
                    .map(|result| result.expect("Dream frontier pair was not evaluated"))
                    .collect()
            })
            .collect())
    }
}
