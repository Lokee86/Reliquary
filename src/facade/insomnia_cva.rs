use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeId, EpisodeOrigin, EpisodePolicy,
    EpisodeSchedulingResult, InsomniaAttempt, InsomniaError, InsomniaLeaseToken, InsomniaPriority,
    InsomniaStats, InsomniaWork, MemoryId,
};

impl Cva {
    pub fn queue_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        priority: InsomniaPriority,
        now_ns: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        self.runtime_queue_insomnia_episode(episode_id, priority, now_ns)
    }

    pub fn materialize_live_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let policy = self.runtime_validate_insomnia_episode_policy(policy)?;
        let episodes = self
            .materialize_path_episodes(
                conversation_id,
                leaf_node_id,
                policy.episode,
                EpisodeOrigin::Live,
                None,
            )
            .map_err(|_| InsomniaError::InvalidField("episode materialization"))?;
        self.queue_episode_result(episodes, InsomniaPriority::Live, now_ns)
    }

    pub fn finalize_inactive_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<Option<EpisodeSchedulingResult>, InsomniaError> {
        let episodes = self
            .finalize_inactive_path_episodes(
                conversation_id,
                leaf_node_id,
                policy,
                EpisodeOrigin::Live,
                now_ns,
            )
            .map_err(|_| InsomniaError::InvalidField("episode inactivity"))?;
        match episodes {
            Some(episodes) => self
                .queue_episode_result(episodes, InsomniaPriority::Live, now_ns)
                .map(Some),
            None => Ok(None),
        }
    }

    pub fn finalize_explicit_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let policy = self.runtime_validate_insomnia_episode_policy(policy)?;
        let episodes = self
            .finalize_explicit_path(conversation_id, leaf_node_id, policy.episode, now_ns)
            .map_err(|_| InsomniaError::InvalidField("explicit episode"))?;
        self.queue_episode_result(episodes, InsomniaPriority::ImmediateLive, now_ns)
    }

    pub fn request_create_memory(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let episodes = self
            .finalize_create_memory_path(conversation_id, leaf_node_id, config, now_ns)
            .map_err(|_| InsomniaError::InvalidField("create_memory episode"))?;
        let mut queued = Vec::new();
        for episode in &episodes.created {
            let priority = if episode.boundary == EpisodeBoundary::CreateMemory {
                InsomniaPriority::ImmediateLive
            } else {
                InsomniaPriority::Live
            };
            let (work, _) = self.queue_insomnia_episode(episode.id, priority, now_ns)?;
            queued.push(work);
        }
        Ok(EpisodeSchedulingResult { episodes, queued })
    }

    pub fn finalize_import_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let episodes = self
            .materialize_path_episodes(
                conversation_id,
                leaf_node_id,
                config,
                EpisodeOrigin::Import,
                Some((EpisodeBoundary::ImportEnd, now_ns)),
            )
            .map_err(|_| InsomniaError::InvalidField("import episode"))?;
        self.queue_episode_result(episodes, InsomniaPriority::Import, now_ns)
    }

    pub fn finalize_canonical_imports_and_queue(
        &mut self,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<Vec<EpisodeSchedulingResult>, InsomniaError> {
        let mut branches: Vec<_> = self
            .branches()
            .into_iter()
            .filter(|branch| branch.canonical)
            .collect();
        branches.sort_by(|left, right| {
            left.conversation_id
                .cmp(&right.conversation_id)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut results = Vec::with_capacity(branches.len());
        for branch in branches {
            results.push(self.finalize_import_path_and_queue(
                &branch.conversation_id,
                &branch.leaf_node_id,
                config,
                now_ns,
            )?);
        }
        for episode in self
            .episodes()
            .into_iter()
            .filter(|episode| episode.origin == EpisodeOrigin::Import)
        {
            self.queue_insomnia_episode(episode.id, InsomniaPriority::Import, now_ns)?;
        }
        Ok(results)
    }

    pub fn claim_insomnia_episode(
        &mut self,
        worker_id: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        self.runtime_claim_insomnia_episode(worker_id, now_ns, lease_duration_ns)
    }

    pub fn renew_insomnia_lease(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.runtime_renew_insomnia_lease(episode_id, token, now_ns, lease_duration_ns)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        started_at_ns: i64,
        completed_at_ns: i64,
        extractor_model: String,
        extractor_version: String,
        memory_ids: Vec<MemoryId>,
        rejected_count: u32,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.runtime_complete_insomnia_episode(
            episode_id,
            token,
            started_at_ns,
            completed_at_ns,
            extractor_model,
            extractor_version,
            memory_ids,
            rejected_count,
        )
    }

    pub fn fail_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        started_at_ns: i64,
        failed_at_ns: i64,
        retry_after_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.runtime_fail_insomnia_episode(
            episode_id,
            token,
            started_at_ns,
            failed_at_ns,
            retry_after_ns,
            reason,
        )
    }

    pub fn terminal_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        started_at_ns: i64,
        terminal_at_ns: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.runtime_terminal_insomnia_episode(
            episode_id,
            token,
            started_at_ns,
            terminal_at_ns,
            reason,
        )
    }

    pub fn retry_terminal_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        now_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.runtime_retry_terminal_insomnia_episode(episode_id, now_ns)
    }

    pub fn insomnia_work(&self, episode_id: EpisodeId) -> Option<&InsomniaWork> {
        self.runtime_insomnia_work(episode_id)
    }

    pub fn insomnia_attempts(&self, episode_id: EpisodeId) -> Vec<InsomniaAttempt> {
        self.runtime_insomnia_attempts(episode_id)
    }

    pub fn insomnia_stats(&self) -> InsomniaStats {
        self.runtime_insomnia_stats()
    }

    fn queue_episode_result(
        &mut self,
        episodes: crate::EpisodeBuildResult,
        priority: InsomniaPriority,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let mut queued = Vec::new();
        for episode in &episodes.created {
            let (work, _) = self.queue_insomnia_episode(episode.id, priority, now_ns)?;
            queued.push(work);
        }
        Ok(EpisodeSchedulingResult { episodes, queued })
    }
}
