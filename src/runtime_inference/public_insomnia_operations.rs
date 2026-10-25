//! Canonical Gateway execution for supported Insomnia processing bindings.
use crate::owner_lifecycle::personal_memory_publication::publish_user_application;
use crate::{
    GeneralEndpoint, InsomniaExtractionError, InsomniaExtractor, InsomniaWork, RelExecutionOwner,
};

pub use crate::insomnia::native_processor::{InsomniaProcessError, InsomniaProcessResult};
use crate::insomnia::native_processor::{
    PreparedUserMemory, application, prepare_application_read,
};
pub(crate) use crate::owner_lifecycle::insomnia_application::commit_application;

impl RelExecutionOwner {
    pub(crate) fn runtime_validate_insomnia_episode_policy(
        &self,
        policy: crate::EpisodePolicy,
    ) -> Result<crate::EpisodePolicy, crate::InsomniaError> {
        self.owner.owner_insomnia_validate_episode_policy(policy)
    }

    pub(crate) fn runtime_queue_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        priority: crate::InsomniaPriority,
        now_ns: i64,
    ) -> Result<(crate::InsomniaWork, bool), crate::InsomniaError> {
        self.owner
            .owner_insomnia_queue_episode(episode_id, priority, now_ns)
    }

    pub(crate) fn runtime_claim_insomnia_episode(
        &mut self,
        worker_id: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<Option<crate::InsomniaWork>, crate::InsomniaError> {
        self.owner
            .owner_insomnia_claim_episode(worker_id, now_ns, lease_duration_ns)
    }

    pub(crate) fn runtime_renew_insomnia_lease(
        &mut self,
        episode_id: crate::EpisodeId,
        token: crate::InsomniaLeaseToken,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        self.owner
            .owner_insomnia_renew_lease(episode_id, token, now_ns, lease_duration_ns)
    }

    pub(crate) fn runtime_fail_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        token: crate::InsomniaLeaseToken,
        started_at_ns: i64,
        failed_at_ns: i64,
        retry_after_ns: i64,
        reason: String,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        self.owner.owner_insomnia_fail_episode(
            episode_id,
            token,
            started_at_ns,
            failed_at_ns,
            retry_after_ns,
            reason,
        )
    }

    pub(crate) fn runtime_terminal_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        token: crate::InsomniaLeaseToken,
        started_at_ns: i64,
        terminal_at_ns: i64,
        reason: String,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        self.owner.owner_insomnia_terminal_episode(
            episode_id,
            token,
            started_at_ns,
            terminal_at_ns,
            reason,
        )
    }

    pub(crate) fn runtime_retry_terminal_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        now_ns: i64,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        self.owner
            .owner_insomnia_retry_terminal_episode(episode_id, now_ns)
    }

    pub(crate) fn runtime_insomnia_work(
        &self,
        episode_id: crate::EpisodeId,
    ) -> Option<&crate::InsomniaWork> {
        self.owner.owner_insomnia_work(episode_id)
    }

    pub(crate) fn native_process_claimed_insomnia_episode<E: GeneralEndpoint>(
        &mut self,
        claim: &InsomniaWork,
        extractor: &InsomniaExtractor<E>,
        scope: &str,
        started_at_ns: i64,
        completed_at_ns: i64,
    ) -> Result<InsomniaProcessResult, InsomniaProcessError> {
        let (episode, turns) = self.native_claimed_episode_input(claim, scope)?;
        let extraction = extract_with_evidence(self, extractor, &episode, &turns)?;
        let mut prepared = prepare_application_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            self.runtime_owner_id(),
            &episode,
            &turns,
            extraction,
            scope,
            completed_at_ns,
        )?;
        crate::insomnia::temporal::infer_prepared(extractor.temporal_endpoint(), &mut prepared)?;
        let owner_uuid = self.runtime_owner_uuid();
        let accepted_rel_turn = self.runtime_rel_turn_count();
        commit_application(
            &mut self.owner.container,
            &mut self.owner.memories,
            &mut self.owner.insomnia,
            &mut self.owner.freshness,
            &mut self.owner.dream_freshness,
            owner_uuid,
            accepted_rel_turn,
            claim,
            prepared,
            None,
            started_at_ns,
            completed_at_ns,
        )
    }

    pub(crate) fn native_process_claimed_insomnia_episode_routed<E: GeneralEndpoint>(
        &mut self,
        phylactery: &mut crate::phy_execution_owner::PhyExecutionOwner,
        claim: &InsomniaWork,
        extractor: &InsomniaExtractor<E>,
        scope: &str,
        started_at_ns: i64,
        completed_at_ns: i64,
    ) -> Result<InsomniaProcessResult, InsomniaProcessError> {
        let (episode, turns) = self.native_claimed_episode_input(claim, scope)?;
        let extraction = extract_with_evidence(self, extractor, &episode, &turns)?;
        let mut prepared = prepare_application_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            self.runtime_owner_id(),
            &episode,
            &turns,
            extraction,
            scope,
            completed_at_ns,
        )?;
        crate::insomnia::temporal::infer_prepared(extractor.temporal_endpoint(), &mut prepared)?;
        let user_publication = if prepared.user_drafts.is_empty() {
            None
        } else {
            Some(publish_user_application(&mut crate::owner_lifecycle::personal_memory_publication::PersonalMemoryPublication::new(&mut phylactery.owner.container,&mut phylactery.owner.memories), &prepared.user_drafts)?)
        };
        let owner_uuid = self.runtime_owner_uuid();
        let accepted_rel_turn = self.runtime_rel_turn_count();
        commit_application(
            &mut self.owner.container,
            &mut self.owner.memories,
            &mut self.owner.insomnia,
            &mut self.owner.freshness,
            &mut self.owner.dream_freshness,
            owner_uuid,
            accepted_rel_turn,
            claim,
            prepared,
            user_publication,
            started_at_ns,
            completed_at_ns,
        )
    }

    pub(crate) fn native_claimed_episode_input(
        &mut self,
        claim: &InsomniaWork,
        scope: &str,
    ) -> Result<(crate::Episode, Vec<crate::ResolvedTurn>), InsomniaProcessError> {
        crate::insomnia::native_processor::claimed_episode_input_read(
            &crate::archive::ArchiveReadView::new(&self.archive, &self.container),
            claim,
            scope,
        )
    }
}

pub(crate) fn extract_with_evidence<E: GeneralEndpoint>(
    cva: &mut RelExecutionOwner,
    extractor: &InsomniaExtractor<E>,
    episode: &crate::Episode,
    turns: &[crate::ResolvedTurn],
) -> Result<crate::InsomniaExtraction, InsomniaExtractionError> {
    use crate::insomnia::InsomniaExtractionStage;

    match extractor.start(episode, turns)? {
        InsomniaExtractionStage::Complete(extraction) => Ok(extraction),
        InsomniaExtractionStage::Evidence(round) => {
            let (results, evidence_turns) = crate::insomnia::resolve_evidence_parts(
                &crate::archive::ArchiveReadView::new(&cva.owner.archive, &cva.owner.container),
                &mut cva.lexical_index,
                episode,
                &round.requests,
            )?;
            extractor.finish_evidence(episode, turns, round, results, evidence_turns)
        }
    }
}
