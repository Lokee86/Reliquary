use crate::{
    EpisodeId, EpisodePolicy, InsomniaAttempt, InsomniaError, InsomniaLeaseToken, InsomniaPriority,
    InsomniaWork,
};

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn owner_insomnia_validate_episode_policy(
        &self,
        policy: EpisodePolicy,
    ) -> Result<EpisodePolicy, InsomniaError> {
        policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode policy"))
    }

    pub(crate) fn owner_insomnia_queue_episode(
        &mut self,
        episode_id: EpisodeId,
        priority: InsomniaPriority,
        now_ns: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        let episode = self
            .archive
            .episode(episode_id)
            .ok_or(InsomniaError::MissingEpisode)?;
        let source = crate::owner_lifecycle::insomnia_replay::episode_source(&episode, priority);
        self.insomnia.queue(&mut self.container, &source, now_ns)
    }

    pub(crate) fn owner_insomnia_claim_episode(
        &mut self,
        worker_id: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        self.insomnia
            .claim_next(&mut self.container, worker_id, now_ns, lease_duration_ns)
    }

    pub(crate) fn owner_insomnia_renew_lease(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.insomnia.renew(
            &mut self.container,
            episode_id,
            token,
            now_ns,
            lease_duration_ns,
        )
    }

    pub(crate) fn owner_insomnia_fail_episode(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        started_at_ns: i64,
        failed_at_ns: i64,
        retry_after_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.insomnia.fail(
            &mut self.container,
            episode_id,
            token,
            started_at_ns,
            failed_at_ns,
            retry_after_ns,
            reason,
        )
    }

    pub(crate) fn owner_insomnia_terminal_episode(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        started_at_ns: i64,
        terminal_at_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.insomnia.terminal(
            &mut self.container,
            episode_id,
            token,
            started_at_ns,
            terminal_at_ns,
            reason,
        )
    }

    pub(crate) fn owner_insomnia_retry_terminal_episode(
        &mut self,
        episode_id: EpisodeId,
        now_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.insomnia
            .retry_terminal(&mut self.container, episode_id, now_ns)
    }

    pub(crate) fn owner_insomnia_work(&self, episode_id: EpisodeId) -> Option<&InsomniaWork> {
        self.insomnia.work(episode_id)
    }

    pub(crate) fn owner_insomnia_attempts(&self, episode_id: EpisodeId) -> Vec<InsomniaAttempt> {
        self.insomnia.attempts(episode_id)
    }
}
