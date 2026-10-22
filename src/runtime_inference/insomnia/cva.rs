use super::completion::{InsomniaCompletion, encode_completion};
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
        if self.archive.episode(episode_id).is_none() {
            return Err(InsomniaError::MissingEpisode);
        }
        self.insomnia.queue(
            &mut self.container,
            &self.archive,
            episode_id,
            priority,
            now_ns,
        )
    }

    pub fn materialize_live_path_and_queue(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let policy = policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode policy"))?;
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
        let policy = policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode policy"))?;
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
        self.insomnia
            .claim_next(&mut self.container, worker_id, now_ns, lease_duration_ns)
    }

    pub fn renew_insomnia_lease(
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
        if memory_ids
            .iter()
            .any(|id| !self.memories.contains_memory(*id))
        {
            return Err(InsomniaError::InvalidTransition);
        }
        let claim = self
            .insomnia
            .active_claim(episode_id, token, completed_at_ns)?;
        let completion = InsomniaCompletion {
            episode_id,
            attempt: claim.attempt_count,
            started_at_ns,
            completed_at_ns,
            transaction_time_ns: crate::Container::transaction_time_now_ns()?,
            extractor_model,
            extractor_version,
            rejected_count,
            memory_ids,
            external_memory_refs: Vec::new(),
            global_version_start: self.container.next_version_candidate(),
            bodies: Vec::new(),
            records: Vec::new(),
            routing_metadata: Vec::new(),
            freshness_birth_turn: None,
            freshness_policy_version: None,
            freshness_birth_memory_ids: Vec::new(),
        };
        let payload = encode_completion(&completion)
            .map_err(|_| InsomniaError::InvalidField("completion record"))?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.insomnia.apply_completion(&completion)?;
        self.insomnia
            .work(episode_id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)
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

    pub fn terminal_insomnia_episode(
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

    pub fn retry_terminal_insomnia_episode(
        &mut self,
        episode_id: EpisodeId,
        now_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.insomnia
            .retry_terminal(&mut self.container, episode_id, now_ns)
    }

    pub fn insomnia_work(&self, episode_id: EpisodeId) -> Option<&InsomniaWork> {
        self.insomnia.work(episode_id)
    }

    pub fn insomnia_attempts(&self, episode_id: EpisodeId) -> Vec<InsomniaAttempt> {
        self.insomnia.attempts(episode_id)
    }

    pub fn insomnia_stats(&self) -> InsomniaStats {
        self.insomnia.stats()
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
