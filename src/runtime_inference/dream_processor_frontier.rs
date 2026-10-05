use crate::{
    CompatibilityProfileId, Cva, DreamCandidateConfig, DreamCandidateSet, DreamPairClassification,
    DreamPairVerification, DreamProcessError, DreamProcessResult, DreamProcessedPair,
    DreamProcessor, DreamVerificationPolicy, GeneralEndpoint, MemoryId, Phylactery,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

pub const DEFAULT_DREAM_FRONTIER_SIZE: usize = 6;
pub const DEFAULT_DREAM_INFERENCE_CONCURRENCY: usize = 12;

#[derive(Debug)]
pub struct DreamMemoryProcessOutcome {
    pub memory_id: MemoryId,
    pub result: Result<DreamProcessResult, DreamProcessError>,
}

type PairEvaluation =
    Result<(DreamPairClassification, Option<DreamPairVerification>), DreamProcessError>;

impl<C: GeneralEndpoint, V: GeneralEndpoint> DreamProcessor<C, V> {
    /// Processes owner-local Memories in deterministic frontiers.
    ///
    /// Candidate sets in one frontier are selected from the same pre-frontier graph state. Pair
    /// inference is globally bounded across the frontier, then successful Memories are published
    /// in `source_ids` order. Classification or verification failure leaves that Memory untouched.
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
        let mut outcomes = Vec::with_capacity(source_ids.len());
        for frontier in source_ids.chunks(frontier_size.max(1)) {
            let staged = frontier
                .iter()
                .copied()
                .map(|source_id| {
                    Ok((
                        source_id,
                        cva.dream_candidates(
                            compatibility_profile_id,
                            source_id,
                            candidate_config,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, DreamProcessError>>()?;
            let evaluated =
                self.evaluate_frontier(&staged, verification_policy, inference_concurrency.max(1))?;

            for ((source_id, candidates), pair_results) in staged.into_iter().zip(evaluated) {
                let candidate_count = candidates.candidates.len();
                let pairs = match inference_results(pair_results)? {
                    Ok(pairs) => pairs,
                    Err(error) => {
                        outcomes.push(DreamMemoryProcessOutcome {
                            memory_id: source_id,
                            result: Err(error),
                        });
                        continue;
                    }
                };
                crate::dream_freshness::begin_initial_pass(
                    cva,
                    source_id,
                    candidates
                        .candidates
                        .iter()
                        .map(|candidate| candidate.context.memory.id),
                )
                .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
                let mut processed = Vec::with_capacity(candidate_count);
                for (classification, verification) in pairs {
                    let publication = cva.publish_dream_pair(
                        &classification,
                        verification.as_ref(),
                        verification_policy,
                        cva.graph_version(),
                    )?;
                    crate::dream_freshness::record_publication_roots(cva, source_id, &publication)
                        .map_err(|error| DreamProcessError::Freshness(error.to_string()))?;
                    cva.mark_dream_pair_evaluated(classification.a, classification.b)?;
                    processed.push(DreamProcessedPair {
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
                outcomes.push(DreamMemoryProcessOutcome {
                    memory_id: source_id,
                    result: Ok(DreamProcessResult {
                        source: lifecycle.source.clone(),
                        candidate_count,
                        pairs: processed,
                        lifecycle,
                    }),
                });
            }
        }
        Ok(outcomes)
    }

    /// Phylactery equivalent of [`Self::process_memories_with_concurrency`].
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
        let mut outcomes = Vec::with_capacity(source_ids.len());
        for frontier in source_ids.chunks(frontier_size.max(1)) {
            let staged = frontier
                .iter()
                .copied()
                .map(|source_id| {
                    Ok((
                        source_id,
                        phylactery.dream_candidates(
                            compatibility_profile_id,
                            source_id,
                            candidate_config,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, DreamProcessError>>()?;
            let evaluated =
                self.evaluate_frontier(&staged, verification_policy, inference_concurrency.max(1))?;

            for ((source_id, candidates), pair_results) in staged.into_iter().zip(evaluated) {
                let candidate_count = candidates.candidates.len();
                let pairs = match inference_results(pair_results)? {
                    Ok(pairs) => pairs,
                    Err(error) => {
                        outcomes.push(DreamMemoryProcessOutcome {
                            memory_id: source_id,
                            result: Err(error),
                        });
                        continue;
                    }
                };
                let mut processed = Vec::with_capacity(candidate_count);
                for (classification, verification) in pairs {
                    let publication = phylactery.publish_dream_pair(
                        &classification,
                        verification.as_ref(),
                        verification_policy,
                        phylactery.graph_version(),
                    )?;
                    phylactery.mark_dream_pair_evaluated(classification.a, classification.b)?;
                    processed.push(DreamProcessedPair {
                        classification,
                        verification,
                        publication,
                    });
                }
                let lifecycle = phylactery.reconcile_dream_lifecycle(source_id)?;
                outcomes.push(DreamMemoryProcessOutcome {
                    memory_id: source_id,
                    result: Ok(DreamProcessResult {
                        source: lifecycle.source.clone(),
                        candidate_count,
                        pairs: processed,
                        lifecycle,
                    }),
                });
            }
        }
        Ok(outcomes)
    }

    fn evaluate_frontier(
        &self,
        staged: &[(MemoryId, DreamCandidateSet)],
        verification_policy: DreamVerificationPolicy,
        inference_concurrency: usize,
    ) -> Result<Vec<Vec<PairEvaluation>>, DreamProcessError> {
        let mut work = Vec::new();
        let max_candidates = staged
            .iter()
            .map(|(_, candidates)| candidates.candidates.len())
            .max()
            .unwrap_or(0);
        for candidate_index in 0..max_candidates {
            for (memory_index, (_, candidates)) in staged.iter().enumerate() {
                if let Some(candidate) = candidates.candidates.get(candidate_index) {
                    work.push((
                        memory_index,
                        candidate_index,
                        &candidates.source,
                        &candidate.context,
                    ));
                }
            }
        }

        let mut evaluated: Vec<Vec<Option<PairEvaluation>>> = staged
            .iter()
            .map(|(_, candidates)| {
                std::iter::repeat_with(|| None)
                    .take(candidates.candidates.len())
                    .collect()
            })
            .collect();

        let worker_count = inference_concurrency.min(work.len());
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
                                    .is_err_and(DreamProcessError::is_backpressure);
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

fn inference_results(
    results: Vec<PairEvaluation>,
) -> Result<
    Result<Vec<(DreamPairClassification, Option<DreamPairVerification>)>, DreamProcessError>,
    DreamProcessError,
> {
    let mut pairs = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(pair) => pairs.push(pair),
            Err(
                error @ (DreamProcessError::Classification(_) | DreamProcessError::Verification(_)),
            ) => {
                return Ok(Err(error));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(Ok(pairs))
}
