use super::progress::{elapsed_ms_between, report_retry};
use super::{DrainCounters, DrainShared, now_ns};
use crate::insomnia::backpressure;
use crate::insomnia::processor::{
    commit_application, prepare_application, publish_user_application,
};
use crate::{
    GeneralEndpointError, InsomniaError, InsomniaExtractionError, InsomniaProgressEvent,
    InsomniaWork,
};
use std::sync::atomic::Ordering;

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_success(
    shared: &DrainShared<'_>,
    claim: &InsomniaWork,
    episode: &crate::Episode,
    turns: &[crate::ResolvedTurn],
    extraction: crate::InsomniaExtraction,
    started_at_ns: i64,
    config: &crate::InsomniaWorkerConfig,
    counters: &DrainCounters,
) -> Result<(), crate::InsomniaWorkerError> {
    let evidence_turns = extraction.evidence_turns.len();
    counters
        .evidence_turns
        .fetch_add(evidence_turns, Ordering::Relaxed);
    let completed_at_ns = now_ns();
    {
        let mut container = shared
            .container
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut insomnia = shared
            .insomnia
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        insomnia.renew(
            &mut container,
            claim.episode_id,
            claim.lease_token.unwrap(),
            completed_at_ns,
            config.lease_duration_ns,
        )?;
    }
    let prepared = {
        let mut container = shared
            .container
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        prepare_application(
            shared.archive,
            &mut container,
            episode,
            turns,
            extraction,
            &config.scope,
            completed_at_ns,
        )?
    };
    let user_publication = if prepared.user_drafts.is_empty() {
        None
    } else {
        let phylactery = shared.phylactery.as_ref().ok_or_else(|| {
            crate::InsomniaWorkerError::Process(crate::InsomniaProcessError::UserRoutingRequired)
        })?;
        let mut phylactery = phylactery
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        Some(publish_user_application(
            &mut phylactery,
            &prepared.user_drafts,
        )?)
    };
    let result = {
        let mut container = shared
            .container
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut memories = shared
            .memories
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut insomnia = shared
            .insomnia
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut freshness = shared
            .freshness
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut dream_freshness = shared
            .dream_freshness
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        commit_application(
            &mut container,
            &mut memories,
            &mut insomnia,
            &mut freshness,
            &mut dream_freshness,
            shared.owner_uuid,
            shared.accepted_rel_turn,
            claim,
            prepared,
            user_publication,
            started_at_ns,
            completed_at_ns,
        )?
    };
    let project_created = result.created.len();
    let project_existing = result.existing.len();
    let user_created = result.user_created.len();
    let user_existing = result.user_existing.len();
    let rejected = result.rejected.len();
    counters.completed.fetch_add(1, Ordering::Relaxed);
    counters
        .created
        .fetch_add(project_created, Ordering::Relaxed);
    counters
        .existing
        .fetch_add(project_existing, Ordering::Relaxed);
    counters
        .user_created
        .fetch_add(user_created, Ordering::Relaxed);
    counters
        .user_existing
        .fetch_add(user_existing, Ordering::Relaxed);
    counters.rejected.fetch_add(rejected, Ordering::Relaxed);
    let (total, complete, terminal) = shared.progress.counts(counters);
    shared
        .progress
        .report(InsomniaProgressEvent::EpisodeCompleted {
            episode_id: claim.episode_id,
            attempt: claim.attempt_count,
            elapsed_ms: elapsed_ms_between(started_at_ns, completed_at_ns),
            total,
            complete,
            terminal,
            project_created,
            project_existing,
            user_created,
            user_existing,
            rejected,
            evidence_turns,
        });
    Ok(())
}

pub(super) fn record_failure(
    shared: &DrainShared<'_>,
    claim: &InsomniaWork,
    started_at_ns: i64,
    error: InsomniaExtractionError,
    config: &crate::InsomniaWorkerConfig,
    counters: &DrainCounters,
) -> Result<(), crate::InsomniaWorkerError> {
    let failed_at_ns = now_ns();
    let reason = bounded_reason(error.to_string());
    if let Some(retry_after_ns) = backpressure::retry_after_ns(&error) {
        {
            let mut container = shared
                .container
                .lock()
                .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
            let mut insomnia = shared
                .insomnia
                .lock()
                .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
            insomnia.fail(
                &mut container,
                claim.episode_id,
                claim.lease_token.unwrap(),
                started_at_ns,
                failed_at_ns,
                failed_at_ns.saturating_add(retry_after_ns),
                reason.clone(),
            )?;
        }
        counters.failed.fetch_add(1, Ordering::Relaxed);
        counters
            .paused_for_backpressure
            .store(true, Ordering::SeqCst);
        report_retry(
            shared,
            claim,
            started_at_ns,
            failed_at_ns,
            retry_after_ns,
            true,
            reason,
            counters,
        );
    } else if retryable(&error) && claim.attempt_count < config.max_attempts {
        {
            let mut container = shared
                .container
                .lock()
                .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
            let mut insomnia = shared
                .insomnia
                .lock()
                .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
            insomnia.fail(
                &mut container,
                claim.episode_id,
                claim.lease_token.unwrap(),
                started_at_ns,
                failed_at_ns,
                failed_at_ns.saturating_add(config.retry_delay_ns),
                reason.clone(),
            )?;
        }
        counters.failed.fetch_add(1, Ordering::Relaxed);
        report_retry(
            shared,
            claim,
            started_at_ns,
            failed_at_ns,
            config.retry_delay_ns,
            false,
            reason,
            counters,
        );
    } else {
        terminal_claim(shared, claim, started_at_ns, reason, counters)?;
    }
    Ok(())
}

pub(super) fn terminal_claim(
    shared: &DrainShared<'_>,
    claim: &InsomniaWork,
    started_at_ns: i64,
    reason: String,
    counters: &DrainCounters,
) -> Result<(), crate::InsomniaWorkerError> {
    let terminal_at_ns = now_ns();
    let error = bounded_reason(reason);
    {
        let mut container = shared
            .container
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        let mut insomnia = shared
            .insomnia
            .lock()
            .map_err(|_| crate::InsomniaWorkerError::LockPoisoned)?;
        insomnia.terminal(
            &mut container,
            claim.episode_id,
            claim.lease_token.ok_or(InsomniaError::InvalidLease)?,
            started_at_ns,
            terminal_at_ns,
            error.clone(),
        )?;
    }
    counters.terminal.fetch_add(1, Ordering::Relaxed);
    let (total, complete, terminal) = shared.progress.counts(counters);
    shared
        .progress
        .report(InsomniaProgressEvent::EpisodeTerminal {
            episode_id: claim.episode_id,
            attempt: claim.attempt_count,
            elapsed_ms: elapsed_ms_between(started_at_ns, terminal_at_ns),
            error,
            total,
            complete,
            terminal,
        });
    Ok(())
}

fn retryable(error: &InsomniaExtractionError) -> bool {
    matches!(
        error,
        InsomniaExtractionError::InvalidOutput(_)
            | InsomniaExtractionError::Endpoint(GeneralEndpointError::Failure(_))
            | InsomniaExtractionError::Endpoint(GeneralEndpointError::InvalidResponse(_))
    )
}

fn bounded_reason(mut reason: String) -> String {
    reason = reason.trim().to_owned();
    if reason.is_empty() {
        return "Insomnia processing failed".into();
    }
    reason.truncate(4096);
    reason
}
