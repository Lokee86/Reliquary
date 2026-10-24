use super::RuntimeInsomniaClaim;
use super::backpressure;
use super::evidence::{hydrate_evidence_parts, plan_evidence};
use super::extraction::{InsomniaEvidenceRound, InsomniaExtractionError};
use super::native_processor::{
    PreparedApplication, UserPublication, claimed_episode_input_parts, prepare_application,
};
use super::store::InsomniaStore;
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_store::MemoryStore;
use crate::{
    Archive, Container, InsomniaExtraction, InsomniaProcessResult, InsomniaWorkerConfig,
    InsomniaWorkerError,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn claim_runtime_insomnia(
    container: &mut Container,
    archive: &Archive,
    insomnia: &mut InsomniaStore,
    worker_id: &str,
    config: &InsomniaWorkerConfig,
) -> Result<Option<RuntimeInsomniaClaim>, InsomniaWorkerError> {
    let started_at_ns = now_ns();
    let Some(work) = insomnia.claim_next(
        container,
        worker_id,
        started_at_ns,
        config.lease_duration_ns,
    )?
    else {
        return Ok(None);
    };
    match claimed_episode_input_parts(archive, container, &work, &config.scope) {
        Ok((episode, turns)) => Ok(Some(RuntimeInsomniaClaim {
            work,
            episode,
            turns,
            started_at_ns,
        })),
        Err(error) => {
            insomnia.terminal(
                container,
                work.episode_id,
                work.lease_token.ok_or(crate::InsomniaError::InvalidLease)?,
                started_at_ns,
                now_ns(),
                bounded_reason(error.to_string()),
            )?;
            Ok(None)
        }
    }
}

pub(crate) fn hydrate_runtime_insomnia_evidence(
    archive: &Archive,
    lexical_index: &mut LexicalIndex,
    container: &mut Container,
    claim: &RuntimeInsomniaClaim,
    round: &InsomniaEvidenceRound,
) -> Result<
    (
        Vec<crate::InsomniaEvidenceResult>,
        Vec<crate::InsomniaEvidenceTurn>,
    ),
    InsomniaExtractionError,
> {
    let archive_read = crate::archive::ArchiveReadView::new(archive, container);
    lexical_index
        .ensure_current(&archive_read)
        .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
    let plans = plan_evidence(archive, lexical_index, &claim.episode, &round.requests);
    hydrate_evidence_parts(archive, container, &plans)
}

pub(crate) fn prepare_runtime_insomnia_application(
    container: &mut Container,
    archive: &Archive,
    insomnia: &mut InsomniaStore,
    claim: &RuntimeInsomniaClaim,
    extraction: InsomniaExtraction,
    config: &InsomniaWorkerConfig,
) -> Result<PreparedApplication, InsomniaWorkerError> {
    let prepared_at_ns = now_ns();
    insomnia.renew(
        container,
        claim.work.episode_id,
        claim
            .work
            .lease_token
            .ok_or(crate::InsomniaError::InvalidLease)?,
        prepared_at_ns,
        config.lease_duration_ns,
    )?;
    prepare_application(
        archive,
        container,
        &claim.episode,
        &claim.turns,
        extraction,
        &config.scope,
        prepared_at_ns,
    )
    .map_err(Into::into)
}

pub(crate) fn commit_runtime_insomnia_verified(
    container: &mut Container,
    archive: &Archive,
    memories: &mut MemoryStore,
    insomnia: &mut InsomniaStore,
    freshness: &mut FreshnessStore,
    dream_freshness: &mut DreamFreshnessJournal,
    claim: &RuntimeInsomniaClaim,
    prepared: PreparedApplication,
    publication: UserPublication,
    config: &InsomniaWorkerConfig,
) -> Result<InsomniaProcessResult, InsomniaWorkerError> {
    let completed_at_ns = now_ns();
    insomnia.renew(
        container,
        claim.work.episode_id,
        claim
            .work
            .lease_token
            .ok_or(crate::InsomniaError::InvalidLease)?,
        completed_at_ns,
        config.lease_duration_ns,
    )?;
    let owner_uuid = container.owner_uuid();
    let accepted_rel_turn = archive.rel_turn_count();
    crate::owner_lifecycle::insomnia_application::commit_application(
        container,
        memories,
        insomnia,
        freshness,
        dream_freshness,
        owner_uuid,
        accepted_rel_turn,
        &claim.work,
        prepared,
        Some(publication),
        claim.started_at_ns,
        completed_at_ns,
    )
    .map_err(Into::into)
}

pub(crate) fn fail_runtime_insomnia(
    container: &mut Container,
    insomnia: &mut InsomniaStore,
    claim: &RuntimeInsomniaClaim,
    error: &InsomniaExtractionError,
    config: &InsomniaWorkerConfig,
) -> Result<(), InsomniaWorkerError> {
    let failed_at_ns = now_ns();
    let reason = bounded_reason(error.to_string());
    let token = claim
        .work
        .lease_token
        .ok_or(crate::InsomniaError::InvalidLease)?;
    if let Some(retry_after_ns) = backpressure::retry_after_ns(error) {
        insomnia.fail(
            container,
            claim.work.episode_id,
            token,
            claim.started_at_ns,
            failed_at_ns,
            failed_at_ns.saturating_add(retry_after_ns),
            reason,
        )?;
    } else if retryable(error) && claim.work.attempt_count < config.max_attempts {
        insomnia.fail(
            container,
            claim.work.episode_id,
            token,
            claim.started_at_ns,
            failed_at_ns,
            failed_at_ns.saturating_add(config.retry_delay_ns),
            reason,
        )?;
    } else {
        insomnia.terminal(
            container,
            claim.work.episode_id,
            token,
            claim.started_at_ns,
            failed_at_ns,
            reason,
        )?;
    }
    Ok(())
}

fn retryable(error: &InsomniaExtractionError) -> bool {
    !matches!(
        error,
        InsomniaExtractionError::Endpoint(crate::GeneralEndpointError::InvalidConfiguration(_))
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

pub(crate) fn now_ns() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    i64::try_from(nanos).unwrap_or(i64::MAX)
}
