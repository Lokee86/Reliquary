use super::backpressure;
use super::evidence::{hydrate_evidence_parts, plan_evidence};
use super::extraction::{InsomniaEvidenceRound, InsomniaExtractionError};
use super::processor::{
    PreparedApplication, commit_application, prepare_application, publish_user_application,
};
use crate::{
    Cva, InsomniaExtraction, InsomniaProcessResult, InsomniaWork, InsomniaWorkerConfig,
    InsomniaWorkerError, Phylactery,
};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct RuntimeInsomniaClaim {
    pub(crate) work: InsomniaWork,
    pub(crate) episode: crate::Episode,
    pub(crate) turns: Vec<crate::ResolvedTurn>,
    pub(crate) started_at_ns: i64,
}

impl Cva {
    pub(crate) fn claim_runtime_insomnia(
        &mut self,
        worker_id: &str,
        config: &InsomniaWorkerConfig,
    ) -> Result<Option<RuntimeInsomniaClaim>, InsomniaWorkerError> {
        let started_at_ns = now_ns();
        let Some(work) =
            self.claim_insomnia_episode(worker_id, started_at_ns, config.lease_duration_ns)?
        else {
            return Ok(None);
        };
        match self.claimed_episode_input(&work, &config.scope) {
            Ok((episode, turns)) => Ok(Some(RuntimeInsomniaClaim {
                work,
                episode,
                turns,
                started_at_ns,
            })),
            Err(error) => {
                self.terminal_insomnia_episode(
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
        &mut self,
        claim: &RuntimeInsomniaClaim,
        round: &InsomniaEvidenceRound,
    ) -> Result<
        (
            Vec<crate::InsomniaEvidenceResult>,
            Vec<crate::InsomniaEvidenceTurn>,
        ),
        InsomniaExtractionError,
    > {
        self.lexical_index
            .ensure_current(&self.archive, &mut self.container)
            .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
        let plans = plan_evidence(
            &self.archive,
            &self.lexical_index,
            &claim.episode,
            &round.requests,
        );
        hydrate_evidence_parts(&self.archive, &mut self.container, &plans)
    }

    pub(crate) fn prepare_runtime_insomnia_application(
        &mut self,
        claim: &RuntimeInsomniaClaim,
        extraction: InsomniaExtraction,
        config: &InsomniaWorkerConfig,
    ) -> Result<PreparedApplication, InsomniaWorkerError> {
        let prepared_at_ns = now_ns();
        self.renew_insomnia_lease(
            claim.work.episode_id,
            claim
                .work
                .lease_token
                .ok_or(crate::InsomniaError::InvalidLease)?,
            prepared_at_ns,
            config.lease_duration_ns,
        )?;
        prepare_application(
            &self.archive,
            &mut self.container,
            &claim.episode,
            &claim.turns,
            extraction,
            &config.scope,
            prepared_at_ns,
        )
        .map_err(Into::into)
    }

    pub(crate) fn commit_runtime_insomnia_prepared(
        &mut self,
        phylactery: Option<&mut Phylactery>,
        claim: &RuntimeInsomniaClaim,
        prepared: PreparedApplication,
        config: &InsomniaWorkerConfig,
    ) -> Result<InsomniaProcessResult, InsomniaWorkerError> {
        let completed_at_ns = now_ns();
        self.renew_insomnia_lease(
            claim.work.episode_id,
            claim
                .work
                .lease_token
                .ok_or(crate::InsomniaError::InvalidLease)?,
            completed_at_ns,
            config.lease_duration_ns,
        )?;
        let user_publication = match phylactery {
            Some(phylactery) if !prepared.user_drafts.is_empty() => {
                Some(publish_user_application(phylactery, &prepared.user_drafts)?)
            }
            _ => None,
        };
        let owner_uuid = self.owner_uuid();
        let accepted_rel_turn = self.rel_turn_count();
        commit_application(
            &mut self.container,
            &mut self.memories,
            &mut self.insomnia,
            &mut self.freshness,
            &mut self.dream_freshness,
            owner_uuid,
            accepted_rel_turn,
            &claim.work,
            prepared,
            user_publication,
            claim.started_at_ns,
            completed_at_ns,
        )
        .map_err(Into::into)
    }

    pub(crate) fn fail_runtime_insomnia(
        &mut self,
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
            self.fail_insomnia_episode(
                claim.work.episode_id,
                token,
                claim.started_at_ns,
                failed_at_ns,
                failed_at_ns.saturating_add(retry_after_ns),
                reason,
            )?;
        } else if retryable(error) && claim.work.attempt_count < config.max_attempts {
            self.fail_insomnia_episode(
                claim.work.episode_id,
                token,
                claim.started_at_ns,
                failed_at_ns,
                failed_at_ns.saturating_add(config.retry_delay_ns),
                reason,
            )?;
        } else {
            self.terminal_insomnia_episode(
                claim.work.episode_id,
                token,
                claim.started_at_ns,
                failed_at_ns,
                reason,
            )?;
        }
        Ok(())
    }
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
