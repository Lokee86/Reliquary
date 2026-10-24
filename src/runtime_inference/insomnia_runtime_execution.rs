use super::RuntimeInsomniaClaim;
use super::backpressure;
use super::evidence::{hydrate_evidence_read, plan_evidence};
use super::extraction::{InsomniaEvidenceRound, InsomniaExtractionError};
use super::native_processor::{
    PreparedApplication, UserPublication, claimed_episode_input_read, prepare_application_read,
};
use crate::lexical_index::LexicalIndex;
use crate::owner_lifecycle::insomnia_runtime::InsomniaRuntimePublication;
use crate::{InsomniaExtraction, InsomniaProcessResult, InsomniaWorkerConfig, InsomniaWorkerError};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn claim_runtime_insomnia(
    owner: &mut InsomniaRuntimePublication<'_>,
    worker_id: &str,
    config: &InsomniaWorkerConfig,
) -> Result<Option<RuntimeInsomniaClaim>, InsomniaWorkerError> {
    let started_at_ns = now_ns();
    let Some(work) = owner.claim(worker_id, started_at_ns, config.lease_duration_ns)? else {
        return Ok(None);
    };
    match claimed_episode_input_read(&owner.read(), &work, &config.scope) {
        Ok((episode, turns)) => Ok(Some(RuntimeInsomniaClaim {
            work,
            episode,
            turns,
            started_at_ns,
        })),
        Err(error) => {
            owner.terminal(
                &work,
                started_at_ns,
                now_ns(),
                bounded_reason(error.to_string()),
            )?;
            Ok(None)
        }
    }
}

pub(crate) fn hydrate_runtime_insomnia_evidence(
    archive: &crate::Archive,
    archive_read: &crate::archive::ArchiveReadView<'_>,
    lexical_index: &mut LexicalIndex,
    claim: &RuntimeInsomniaClaim,
    round: &InsomniaEvidenceRound,
) -> Result<
    (
        Vec<crate::InsomniaEvidenceResult>,
        Vec<crate::InsomniaEvidenceTurn>,
    ),
    InsomniaExtractionError,
> {
    lexical_index
        .ensure_current(archive_read)
        .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
    let plans = plan_evidence(archive, lexical_index, &claim.episode, &round.requests);
    hydrate_evidence_read(archive_read, &plans)
}

pub(crate) fn prepare_runtime_insomnia_application(
    owner: &mut InsomniaRuntimePublication<'_>,
    claim: &RuntimeInsomniaClaim,
    extraction: InsomniaExtraction,
    config: &InsomniaWorkerConfig,
) -> Result<PreparedApplication, InsomniaWorkerError> {
    let prepared_at_ns = now_ns();
    owner.renew(&claim.work, prepared_at_ns, config.lease_duration_ns)?;
    prepare_application_read(
        &owner.read(),
        owner.owner_id(),
        &claim.episode,
        &claim.turns,
        extraction,
        &config.scope,
        prepared_at_ns,
    )
    .map_err(Into::into)
}

pub(crate) fn commit_runtime_insomnia_verified(
    owner: &mut InsomniaRuntimePublication<'_>,
    claim: &RuntimeInsomniaClaim,
    prepared: PreparedApplication,
    publication: UserPublication,
    config: &InsomniaWorkerConfig,
) -> Result<InsomniaProcessResult, InsomniaWorkerError> {
    let completed_at_ns = now_ns();
    owner.renew(&claim.work, completed_at_ns, config.lease_duration_ns)?;
    owner
        .commit(
            &claim.work,
            prepared,
            publication,
            claim.started_at_ns,
            completed_at_ns,
        )
        .map_err(Into::into)
}

pub(crate) fn fail_runtime_insomnia(
    owner: &mut InsomniaRuntimePublication<'_>,
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
        owner.fail(
            &claim.work,
            claim.started_at_ns,
            failed_at_ns,
            failed_at_ns.saturating_add(retry_after_ns),
            reason,
        )?;
    } else if retryable(error) && claim.work.attempt_count < config.max_attempts {
        owner.fail(
            &claim.work,
            claim.started_at_ns,
            failed_at_ns,
            failed_at_ns.saturating_add(config.retry_delay_ns),
            reason,
        )?;
    } else {
        owner.terminal(&claim.work, claim.started_at_ns, failed_at_ns, reason)?;
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

impl super::evidence::EvidenceSearch for crate::lexical_index::LexicalIndex {
    fn ensure_current(
        &mut self,
        archive: &crate::archive::ArchiveReadView<'_>,
    ) -> Result<(), crate::ArchiveError> {
        crate::lexical_index::LexicalIndex::ensure_current(self, archive)
    }

    fn search_fragments(&self, query: &str, limit: usize) -> Vec<crate::Fragment> {
        self.search(&crate::lexical_contract::lexical_terms(query), limit)
            .into_iter()
            .map(|hit| hit.fragment)
            .collect()
    }
}
