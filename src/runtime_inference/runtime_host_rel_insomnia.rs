use super::super::super::{
    ReliquaryRuntimeHostError, notify_work, operation, stopped, wait_for_work,
    wait_for_work_timeout,
};
use super::RelShared;
use crate::insomnia::{InsomniaEvidenceRound, InsomniaExtractionStage, RuntimeInsomniaClaim};
use crate::{GeneralEndpoint, GeneralEndpointError, InsomniaExtractor};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Clone)]
struct SharedGeneralEndpoint(Arc<dyn GeneralEndpoint>);

impl GeneralEndpoint for SharedGeneralEndpoint {
    fn model(&self) -> &str {
        self.0.model()
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.0
            .complete_json(system_prompt, user_payload, schema_name, schema)
    }
}

pub(super) fn worker_loop(
    shared: Arc<RelShared>,
    index: usize,
) -> Result<(), ReliquaryRuntimeHostError> {
    let worker_id = format!("{}-{}", shared.config.worker_id_prefix.trim(), index + 1);
    let mut seen_epoch = 0_u64;
    loop {
        if stopped(&shared.signal)? {
            return Ok(());
        }
        if !shared.insomnia_enabled.load(Ordering::SeqCst) {
            seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
            continue;
        }
        if index == 0 {
            sweep_inactive(&shared)?;
            reconcile_completed(&shared)?;
        }
        if let Some(wait) = backpressure_wait(&shared) {
            seen_epoch = wait_for_work_timeout(&shared.signal, seen_epoch, wait)?;
            continue;
        }
        let routes = shared
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .clone();
        let extractor = routes.insomnia().map(|main| {
            let mut extractor = InsomniaExtractor::new(SharedGeneralEndpoint(Arc::clone(&main)));
            if let Some(metadata) = routes.insomnia_metadata() {
                extractor = extractor.with_metadata_endpoint(SharedGeneralEndpoint(metadata));
            }
            if let Some(entity_extraction) = routes.entity_extraction() {
                extractor =
                    extractor.with_enrichment_endpoint(SharedGeneralEndpoint(entity_extraction));
            }
            if let Some(ownership) = routes.insomnia_ownership() {
                extractor = extractor.with_ownership_endpoint(SharedGeneralEndpoint(ownership));
            }
            if let Some(temporal) = routes.chronos() {
                extractor = extractor.with_temporal_endpoint(SharedGeneralEndpoint(temporal));
            }
            extractor
        });
        if extractor.is_none() && shared.publication.unobserved()?.is_empty() {
            seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
            continue;
        }
        let claim = {
            let Some(_lease) = shared.admit()? else {
                seen_epoch = wait_for_work(&shared.signal, seen_epoch)?;
                continue;
            };
            shared.mutate(
                super::super::mutation_events::RelMutationFamily::Processing,
                |runtime| {
                    let claim = runtime
                        .cva
                        .claim_runtime_insomnia(&worker_id, &shared.config)
                        .map_err(operation)?;
                    Ok((claim, false, vec![]))
                },
            )?
        };
        if let Some(claim) = claim {
            process_claim(&shared, extractor.as_ref(), &claim)?;
            continue;
        }
        seen_epoch = wait_for_work_timeout(&shared.signal, seen_epoch, super::IDLE_POLL)?;
    }
}

fn process_claim(
    shared: &RelShared,
    extractor: Option<&InsomniaExtractor<SharedGeneralEndpoint>>,
    claim: &RuntimeInsomniaClaim,
) -> Result<(), ReliquaryRuntimeHostError> {
    // Durable intent always wins over a new extraction/classification/model call.
    let existing = match shared.publication.plan(&claim) {
        Ok(plan) => plan,
        Err(error) => {
            shared.publication.hold_claim(&claim, &error)?;
            return retry_publication(shared, &claim, &error);
        }
    };
    let plan = if let Some(plan) = existing {
        plan
    } else {
        let Some(extractor) = extractor else {
            let error = operation("Insomnia endpoint unavailable for unfrozen Episode");
            shared.publication.hold_claim(&claim, &error)?;
            return retry_publication(shared, &claim, &error);
        };
        let extraction = match extractor.start(&claim.episode, &claim.turns) {
            Ok(InsomniaExtractionStage::Complete(extraction)) => Ok(extraction),
            Ok(InsomniaExtractionStage::Evidence(round)) => {
                finish_evidence(shared, extractor, &claim, round)
            }
            Err(error) => Err(error),
        };
        let extraction = match extraction {
            Ok(extraction) => extraction,
            Err(error) => return fail_claim(shared, &claim, &error),
        };
        let mut prepared = {
            let Some(_lease) = shared.admit()? else {
                return Ok(());
            };
            shared.mutate(
                super::super::mutation_events::RelMutationFamily::Processing,
                |runtime| {
                    let value = runtime
                        .cva
                        .prepare_runtime_insomnia_application(&claim, extraction, &shared.config)
                        .map_err(operation)?;
                    // Lease renewal has no additional instance-visible state.
                    Ok((value, false, vec![]))
                },
            )?
        };
        if let Err(error) =
            crate::insomnia::temporal::infer_prepared(extractor.temporal_endpoint(), &mut prepared)
        {
            return fail_claim(shared, &claim, &error);
        }
        let Some(_lease) = shared.admit()? else {
            return Ok(());
        };
        shared.mutate(
            super::super::mutation_events::RelMutationFamily::Processing,
            |runtime| {
                let plan = shared.publication.freeze(runtime, &claim, prepared)?;
                Ok((plan, true, vec![]))
            },
        )?
    };
    let committed = (|| {
        // No REL store lock is held while performing exact personal writes.
        let publication = shared.publication.publish(&plan, true)?;
        let prepared = shared.publication.thaw(&plan)?;
        let Some(_lease) = shared.admit()? else {
            return Err(operation("REL publication is fenced"));
        };
        shared.mutate(
            super::super::mutation_events::RelMutationFamily::Background,
            |runtime| {
                runtime
                    .cva
                    .commit_runtime_insomnia_verified(&claim, prepared, publication, &shared.config)
                    .map_err(operation)?;
                if !shared.publication.verify_receipt(runtime, &plan)? {
                    return Err(operation("REL completion receipt is absent after commit"));
                }
                Ok(((), true, vec![]))
            },
        )?;
        shared.publication.observe(&plan)
    })();
    match committed {
        Ok(()) => notify_work(&shared.signal),
        Err(error) => {
            shared.publication.hold(&plan, &error)?;
            retry_publication(shared, &claim, &error)
        }
    }
}

fn retry_publication(
    shared: &RelShared,
    claim: &RuntimeInsomniaClaim,
    error: &ReliquaryRuntimeHostError,
) -> Result<(), ReliquaryRuntimeHostError> {
    // Frozen owner work remains recoverable beyond model retry limits.
    let Some(_lease) = shared.admit()? else {
        return Ok(());
    };
    shared.mutate(
        super::super::mutation_events::RelMutationFamily::Processing,
        |runtime| {
            let now = crate::insomnia::runtime_step::now_ns();
            runtime
                .cva
                .fail_insomnia_episode(
                    claim.work.episode_id,
                    claim
                        .work
                        .lease_token
                        .ok_or_else(|| operation("publication claim lacks lease"))?,
                    claim.started_at_ns,
                    now,
                    now.saturating_add(shared.config.retry_delay_ns),
                    error.to_string(),
                )
                .map(|_| ())
                .map_err(operation)?;
            Ok(((), true, vec![]))
        },
    )
}

pub(super) fn reconcile_completed(shared: &RelShared) -> Result<(), ReliquaryRuntimeHostError> {
    for plan in shared.publication.unobserved()? {
        let completed = {
            let Some(_lease) = shared.admit()? else {
                return Ok(());
            };
            let mut runtime = shared
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            shared.publication.verify_receipt(&mut runtime, &plan)
        };
        match completed {
            Ok(false) => continue,
            Ok(true) => {}
            Err(error) => {
                shared.publication.hold(&plan, &error)?;
                continue;
            }
        }
        // A successful REL receipt does not grant permission to invent missing
        // personal records during recovery. Verify durable results read-only.
        match shared.publication.publish(&plan, false) {
            Ok(_) => {
                let Some(_lease) = shared.admit()? else {
                    return Ok(());
                };
                shared.publication.observe(&plan)?;
            }
            Err(error) => shared.publication.hold(&plan, &error)?,
        }
    }
    Ok(())
}

fn fail_claim(
    shared: &RelShared,
    claim: &RuntimeInsomniaClaim,
    error: &crate::InsomniaExtractionError,
) -> Result<(), ReliquaryRuntimeHostError> {
    let Some(_lease) = shared.admit()? else {
        return Ok(());
    };
    if let Some(retry_after_ns) = crate::insomnia::backpressure::retry_after_ns(error) {
        let until = crate::insomnia::runtime_step::now_ns().saturating_add(retry_after_ns);
        shared
            .insomnia_backpressure_until_ns
            .fetch_max(until, Ordering::SeqCst);
    }
    shared.mutate(
        super::super::mutation_events::RelMutationFamily::Processing,
        |runtime| {
            runtime
                .cva
                .fail_runtime_insomnia(claim, error, &shared.config)
                .map_err(operation)?;
            Ok(((), true, vec![]))
        },
    )
}

fn backpressure_wait(shared: &RelShared) -> Option<Duration> {
    let until = shared.insomnia_backpressure_until_ns.load(Ordering::SeqCst);
    let remaining = until.saturating_sub(crate::insomnia::runtime_step::now_ns());
    (remaining > 0).then(|| Duration::from_nanos(u64::try_from(remaining).unwrap_or(u64::MAX)))
}

fn finish_evidence(
    shared: &RelShared,
    extractor: &InsomniaExtractor<SharedGeneralEndpoint>,
    claim: &RuntimeInsomniaClaim,
    round: InsomniaEvidenceRound,
) -> Result<crate::InsomniaExtraction, crate::InsomniaExtractionError> {
    let (results, evidence_turns) = {
        let mut runtime = shared.runtime.lock().map_err(|_| {
            crate::InsomniaExtractionError::InvalidOutput("runtime lock poisoned".into())
        })?;
        runtime
            .cva
            .hydrate_runtime_insomnia_evidence(claim, &round)?
    };
    extractor.finish_evidence(&claim.episode, &claim.turns, round, results, evidence_turns)
}

fn sweep_inactive(shared: &RelShared) -> Result<(), ReliquaryRuntimeHostError> {
    let Some(_lease) = shared.admit()? else {
        return Ok(());
    };
    shared.mutate(
        super::super::mutation_events::RelMutationFamily::Processing,
        |runtime| {
            let sessions: Vec<_> = runtime
                .sessions
                .iter()
                .filter(|(_, state)| state.leaf_message_id.is_some() && state.in_flight.is_none())
                .map(|(id, _)| id.clone())
                .collect();
            let now = crate::insomnia::runtime_step::now_ns();
            for session_id in sessions {
                runtime
                    .finalize_inactive_session(&session_id, shared.episode_policy, now)
                    .map_err(operation)?;
            }
            Ok(((), false, vec![]))
        },
    )
}

#[cfg(test)]
pub(super) fn replay_claim_for_test(
    shared: &RelShared,
    claim: &RuntimeInsomniaClaim,
    endpoint: Option<Arc<dyn GeneralEndpoint>>,
) -> Result<(), ReliquaryRuntimeHostError> {
    let extractor =
        endpoint.map(|endpoint| InsomniaExtractor::new(SharedGeneralEndpoint(endpoint)));
    process_claim(shared, extractor.as_ref(), claim)
}
