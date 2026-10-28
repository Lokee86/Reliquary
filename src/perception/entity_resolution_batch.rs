use crate::{
    Cva, EntityCandidateConfig, EntityResolutionEngine, EntityResolutionEvaluation,
    EntityResolutionOutcome, EntityResolutionPreparation, EntityResolutionPrepared,
    EntityResolverError, GeneralEndpoint, GeneralEndpointError, MemoryEntityMentionKey, Phylactery,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub struct EntityResolutionBatchOutcome {
    pub outcomes: Vec<EntityResolutionOutcome>,
    pub speculative_evaluations: usize,
    pub reevaluations: usize,
}

enum SpeculativeEvaluation {
    Complete(EntityResolutionOutcome),
    Evaluated(
        EntityResolutionPrepared,
        Result<EntityResolutionEvaluation, EntityResolverError>,
    ),
}

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn resolve_entity_mentions_with_engine_parallel<E: GeneralEndpoint>(
                &mut self,
                engine: &EntityResolutionEngine<E>,
                keys: &[MemoryEntityMentionKey],
                config: EntityCandidateConfig,
                now_ns: i64,
                workers: usize,
            ) -> Result<EntityResolutionBatchOutcome, EntityResolverError> {
                let wave_size = workers.max(1);
                let mut outcomes = Vec::with_capacity(keys.len());
                let mut speculative_evaluations = 0usize;
                let mut reevaluations = 0usize;
                let mut timestamp = now_ns;

                for wave in keys.chunks(wave_size) {
                    let mut prepared = Vec::with_capacity(wave.len());
                    for key in wave {
                        if let Some(outcome) =
                            self.resolve_principal_entity_mention(*key, timestamp)?
                        {
                            prepared.push(EntityResolutionPreparation::Complete(outcome));
                        } else {
                            prepared.push(self.prepare_entity_resolution(*key, config)?);
                        }
                    }
                    let mut slots = evaluate_wave(engine, prepared, &mut speculative_evaluations)?;

                    if let Some(error) = take_first_error(&mut slots) {
                        return Err(error);
                    }

                    for slot in slots {
                        let outcome = match slot.expect("parallel Entity resolution omitted slot") {
                            SpeculativeEvaluation::Complete(outcome) => outcome,
                            SpeculativeEvaluation::Evaluated(old, Ok(evaluation)) => {
                                match self.prepare_entity_resolution(old.key, config)? {
                                    EntityResolutionPreparation::Complete(outcome) => outcome,
                                    EntityResolutionPreparation::Ready(fresh) => {
                                        let evaluation = if same_relevant_snapshot(&old, &fresh) {
                                            evaluation
                                        } else {
                                            reevaluations += 1;
                                            evaluate_prepared_with_retry(engine, &fresh)?
                                        };
                                        timestamp = timestamp.saturating_add(1);
                                        self.commit_entity_resolution(fresh, evaluation, timestamp)?
                                            .ok_or_else(stale_snapshot)?
                                    }
                                }
                            }
                            SpeculativeEvaluation::Evaluated(_, Err(_)) => unreachable!(),
                        };
                        outcomes.push(outcome);
                    }
                }

                Ok(EntityResolutionBatchOutcome {
                    outcomes,
                    speculative_evaluations,
                    reevaluations,
                })
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);

fn evaluate_wave<E: GeneralEndpoint>(
    engine: &EntityResolutionEngine<E>,
    prepared: Vec<EntityResolutionPreparation>,
    speculative_evaluations: &mut usize,
) -> Result<Vec<Option<SpeculativeEvaluation>>, EntityResolverError> {
    let count = prepared.len();
    let mut slots = (0..count).map(|_| None).collect::<Vec<_>>();
    let (sender, receiver) = mpsc::channel();

    thread::scope(|scope| {
        for (index, preparation) in prepared.into_iter().enumerate() {
            match preparation {
                EntityResolutionPreparation::Complete(outcome) => {
                    slots[index] = Some(SpeculativeEvaluation::Complete(outcome));
                }
                EntityResolutionPreparation::Ready(prepared) => {
                    *speculative_evaluations += 1;
                    let sender = sender.clone();
                    scope.spawn(move || {
                        let evaluation = evaluate_prepared_with_retry(engine, &prepared);
                        let _ = sender.send((index, prepared, evaluation));
                    });
                }
            }
        }
        drop(sender);
        for (index, prepared, evaluation) in receiver {
            slots[index] = Some(SpeculativeEvaluation::Evaluated(prepared, evaluation));
        }
    });

    Ok(slots)
}

fn evaluate_prepared_with_retry<E: GeneralEndpoint>(
    engine: &EntityResolutionEngine<E>,
    prepared: &EntityResolutionPrepared,
) -> Result<EntityResolutionEvaluation, EntityResolverError> {
    const MAX_RETRIES: usize = 5;
    for attempt in 0..=MAX_RETRIES {
        match engine
            .evaluate_prepared(&prepared.evaluation_input())
            .map_err(EntityResolverError::from)
        {
            Ok(evaluation) => return Ok(evaluation),
            Err(error) if attempt < MAX_RETRIES && transient_endpoint_error(&error) => {
                let delay = retry_delay(&error, attempt);
                eprintln!(
                    "transient Entity resolution endpoint failure; retry {}/{} after {:?}: {}",
                    attempt + 1,
                    MAX_RETRIES,
                    delay,
                    error
                );
                thread::sleep(delay);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("retry loop always returns")
}

fn transient_endpoint_error(error: &EntityResolverError) -> bool {
    match error {
        EntityResolverError::Endpoint(GeneralEndpointError::Backpressure {
            retry_after, ..
        }) => retry_after.is_none_or(|delay| delay <= Duration::from_secs(60)),
        EntityResolverError::Endpoint(GeneralEndpointError::Failure(message)) => {
            let lower = message.to_ascii_lowercase();
            [
                "http 500",
                "http 502",
                "http 503",
                "http 504",
                "timeout",
                "timed out",
                "connection",
                "disconnect",
                "reset",
                "temporarily unavailable",
                "request failed",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
        }
        _ => false,
    }
}

fn retry_delay(error: &EntityResolverError, attempt: usize) -> Duration {
    if let EntityResolverError::Endpoint(endpoint) = error {
        if let Some(delay) = endpoint.backpressure_retry_after() {
            return delay.min(Duration::from_secs(60));
        }
    }
    Duration::from_secs(2_u64.saturating_pow(attempt as u32).min(30))
}

fn take_first_error(slots: &mut [Option<SpeculativeEvaluation>]) -> Option<EntityResolverError> {
    for slot in slots {
        if matches!(slot, Some(SpeculativeEvaluation::Evaluated(_, Err(_)))) {
            if let Some(SpeculativeEvaluation::Evaluated(_, Err(error))) = slot.take() {
                return Some(error);
            }
        }
    }
    None
}

fn same_relevant_snapshot(
    old: &EntityResolutionPrepared,
    fresh: &EntityResolutionPrepared,
) -> bool {
    old.key == fresh.key
        && old.owner_id == fresh.owner_id
        && old.expected_resolution_revision == fresh.expected_resolution_revision
        && old.candidate_fingerprint == fresh.candidate_fingerprint
        && old.context_fingerprint == fresh.context_fingerprint
}

fn stale_snapshot() -> EntityResolverError {
    EntityResolverError::InvalidOutput("Entity resolution snapshot became stale".into())
}
