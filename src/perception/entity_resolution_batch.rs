use crate::{
    EntityCandidateConfig, EntityResolutionEngine, EntityResolutionEvaluation,
    EntityResolutionOutcome, EntityResolutionPreparation, EntityResolutionPrepared,
    EntityResolverError, GeneralEndpoint, GeneralEndpointError, MemoryEntityMentionKey,
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

pub(crate) enum SpeculativeEvaluation {
    Complete(EntityResolutionOutcome),
    Evaluated(
        EntityResolutionPrepared,
        Result<EntityResolutionEvaluation, EntityResolverError>,
    ),
}

pub(crate) fn evaluate_wave<E: GeneralEndpoint>(
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

pub(crate) fn evaluate_prepared_with_retry<E: GeneralEndpoint>(
    engine: &EntityResolutionEngine<E>,
    prepared: &EntityResolutionPrepared,
) -> Result<EntityResolutionEvaluation, EntityResolverError> {
    const MAX_RETRIES: usize = 5;
    for attempt in 0..=MAX_RETRIES {
        match engine.evaluate_prepared(prepared) {
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

pub(crate) fn take_first_error(
    slots: &mut [Option<SpeculativeEvaluation>],
) -> Option<EntityResolverError> {
    for slot in slots {
        if matches!(slot, Some(SpeculativeEvaluation::Evaluated(_, Err(_)))) {
            if let Some(SpeculativeEvaluation::Evaluated(_, Err(error))) = slot.take() {
                return Some(error);
            }
        }
    }
    None
}

pub(crate) fn same_relevant_snapshot(
    old: &EntityResolutionPrepared,
    fresh: &EntityResolutionPrepared,
) -> bool {
    old.key == fresh.key
        && old.owner_id == fresh.owner_id
        && old.expected_resolution_revision == fresh.expected_resolution_revision
        && old.candidate_fingerprint == fresh.candidate_fingerprint
        && old.context_fingerprint == fresh.context_fingerprint
}

pub(crate) fn stale_snapshot() -> EntityResolverError {
    EntityResolverError::InvalidOutput("Entity resolution snapshot became stale".into())
}
