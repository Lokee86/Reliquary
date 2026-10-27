use crate::{
    Archive, Episode, EpisodeBoundary, EpisodeOrigin, InsomniaAttempt, InsomniaError,
    InsomniaLeaseToken, InsomniaPriority, InsomniaStats, InsomniaWork, InsomniaWorkState, MemoryId,
};
use reliquary_container::Container;
use reliquary_insomnia_store as lower;

/// Converts product-owned Archive/Memory identities at the boundary to the Container-backed store.
pub(crate) struct InsomniaStore {
    inner: lower::InsomniaStore,
}

impl InsomniaStore {
    pub(crate) fn empty() -> Self {
        Self {
            inner: lower::InsomniaStore::empty(),
        }
    }
    pub(crate) fn initialize(&self, c: &mut Container) -> Result<(), InsomniaError> {
        self.inner.initialize(c).map_err(map_error)
    }
    pub(crate) fn queue(
        &mut self,
        c: &mut Container,
        a: &Archive,
        id: crate::EpisodeId,
        p: InsomniaPriority,
        now: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        if let Some(existing) = self.work(id) {
            return Ok((existing, false));
        }
        let e = a.episode(id).ok_or(InsomniaError::MissingEpisode)?;
        self.inner
            .queue(c, order(e), p.into(), now)
            .map(|(w, b)| (from_work(w), b))
            .map_err(map_error)
    }
    pub(crate) fn claim_next(
        &mut self,
        c: &mut Container,
        worker: &str,
        now: i64,
        duration: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        self.inner
            .claim_next(c, worker, now, duration)
            .map(|x| x.map(from_work))
            .map_err(map_error)
    }
    pub(crate) fn renew(
        &mut self,
        c: &mut Container,
        id: crate::EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
        d: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.inner
            .renew(
                c,
                lower::EpisodeId(id.0),
                lower::InsomniaLeaseToken(t.0),
                now,
                d,
            )
            .map(from_work)
            .map_err(map_error)
    }
    pub(crate) fn fail(
        &mut self,
        c: &mut Container,
        id: crate::EpisodeId,
        t: InsomniaLeaseToken,
        s: i64,
        f: i64,
        r: i64,
        e: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.inner
            .fail(
                c,
                lower::EpisodeId(id.0),
                lower::InsomniaLeaseToken(t.0),
                s,
                f,
                r,
                e,
            )
            .map(from_work)
            .map_err(map_error)
    }
    pub(crate) fn terminal(
        &mut self,
        c: &mut Container,
        id: crate::EpisodeId,
        t: InsomniaLeaseToken,
        s: i64,
        at: i64,
        e: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.inner
            .terminal(
                c,
                lower::EpisodeId(id.0),
                lower::InsomniaLeaseToken(t.0),
                s,
                at,
                e,
            )
            .map(from_work)
            .map_err(map_error)
    }
    pub(crate) fn retry_terminal(
        &mut self,
        c: &mut Container,
        id: crate::EpisodeId,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.inner
            .retry_terminal(c, lower::EpisodeId(id.0), now)
            .map(from_work)
            .map_err(map_error)
    }
    pub(crate) fn work(&self, id: crate::EpisodeId) -> Option<InsomniaWork> {
        self.inner
            .work(lower::EpisodeId(id.0))
            .cloned()
            .map(from_work)
    }
    pub(crate) fn attempts(&self, id: crate::EpisodeId) -> Vec<InsomniaAttempt> {
        self.inner
            .attempts(lower::EpisodeId(id.0))
            .into_iter()
            .map(from_attempt)
            .collect()
    }
    pub(crate) fn stats(&self) -> InsomniaStats {
        let s = self.inner.stats();
        InsomniaStats {
            total: s.total,
            pending: s.pending,
            processing: s.processing,
            complete: s.complete,
            failed: s.failed,
            terminal: s.terminal,
            attempts: s.attempts,
        }
    }
    pub(crate) fn apply_work(&mut self, w: InsomniaWork) {
        self.inner.apply_work(to_work(w))
    }
    pub(crate) fn apply_attempt(&mut self, a: InsomniaAttempt) {
        self.inner.apply_attempt(to_attempt(a))
    }
    /// The caller decodes and authenticates the full completion envelope before projecting it here.
    pub(crate) fn apply_completion(
        &mut self,
        c: &super::completion::InsomniaCompletion,
    ) -> Result<(), InsomniaError> {
        self.inner
            .apply_completion(&accepted_completion(c))
            .map_err(map_error)
    }
    pub(crate) fn check_completion(
        &self,
        c: &super::completion::InsomniaCompletion,
    ) -> Result<lower::CompletionCheck, InsomniaError> {
        self.inner
            .check_completion(&accepted_completion(c))
            .map_err(map_error)
    }
    pub(crate) fn preflight_completion(
        &self,
        c: &super::completion::InsomniaCompletion,
    ) -> Result<(), InsomniaError> {
        match self.check_completion(c)? {
            lower::CompletionCheck::Apply => Ok(()),
            lower::CompletionCheck::AlreadyAccepted => Err(InsomniaError::InvalidTransition),
        }
    }
    pub(crate) fn active_claim(
        &self,
        id: crate::EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.inner
            .active_claim(lower::EpisodeId(id.0), lower::InsomniaLeaseToken(t.0), now)
            .map(from_work)
            .map_err(map_error)
    }
    pub(crate) fn apply_encoded_record(&mut self, bytes: &[u8]) -> Result<bool, InsomniaError> {
        self.inner.replay_record(bytes).map_err(map_error)
    }
    pub(crate) fn is_owner_record(bytes: &[u8]) -> Result<bool, InsomniaError> {
        lower::codec::decode_record(bytes)
            .map(|x| x.is_some())
            .map_err(map_error)
    }
    pub(crate) fn is_format(bytes: &[u8]) -> Result<bool, InsomniaError> {
        lower::codec::decode_format(bytes).map_err(map_error)
    }
    pub(crate) fn validate(
        &self,
        a: &Archive,
        m: &crate::memory_store::MemoryStore,
    ) -> Result<(), InsomniaError> {
        let ids = a
            .historical_episodes()
            .into_iter()
            .map(|e| lower::EpisodeId(e.id.0))
            .collect::<std::collections::HashSet<_>>();
        if self.inner.work_episode_ids().any(|id| !ids.contains(&id))
            || self
                .inner
                .all_attempts()
                .any(|x| !ids.contains(&x.episode_id))
        {
            return Err(InsomniaError::MissingEpisode);
        }
        if self
            .inner
            .all_attempts()
            .flat_map(|x| x.memory_ids.iter())
            .any(|id| !m.contains_historical_memory(MemoryId(id.0)))
        {
            return Err(InsomniaError::InvalidTransition);
        }
        Ok(())
    }
    pub(crate) fn rebuild_schedule(&mut self, a: &Archive) -> Result<(), InsomniaError> {
        let episodes = a
            .historical_episodes()
            .into_iter()
            .map(|e| lower::AcceptedEpisodeState {
                order: order(&e),
                priority: priority(&e).into(),
                finalized_at_ns: e.finalized_at_ns,
            })
            .collect::<Vec<_>>();
        self.inner.rebuild_episodes(&episodes);
        // Historical/tombstoned Episodes retain work state but the old scheduler
        // indexed only Episodes still admitted by Archive::episode.
        let orders = a.episodes().iter().map(order).collect::<Vec<_>>();
        self.inner.rebuild_schedule(&orders);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn scheduler_counts(&self) -> (usize, usize, usize) {
        self.inner.scheduler_counts()
    }
}

fn accepted_completion(
    c: &super::completion::InsomniaCompletion,
) -> lower::AcceptedCompletionState {
    lower::AcceptedCompletionState {
        episode_id: lower::EpisodeId(c.episode_id.0),
        attempt: c.attempt,
        started_at_ns: c.started_at_ns,
        completed_at_ns: c.completed_at_ns,
        extractor_model: c.extractor_model.clone(),
        extractor_version: c.extractor_version.clone(),
        memory_ids: c.memory_ids.iter().map(|x| lower::MemoryId(x.0)).collect(),
        external_memory_refs: c
            .external_memory_refs
            .iter()
            .map(|r| lower::ExternalMemoryRef {
                owner_id: r.owner_id.clone(),
                memory_id: lower::MemoryId(r.memory_id.0),
            })
            .collect(),
        rejected_count: c.rejected_count,
    }
}
fn order(e: &Episode) -> lower::AcceptedEpisodeOrder {
    lower::AcceptedEpisodeOrder {
        episode_id: lower::EpisodeId(e.id.0),
        source_through_ns: e.source_through_ns,
        conversation_id: e.conversation_id.clone(),
        start_node_id: e.start_node_id.clone(),
    }
}
fn priority(e: &Episode) -> InsomniaPriority {
    if matches!(
        e.boundary,
        EpisodeBoundary::CreateMemory | EpisodeBoundary::Explicit
    ) {
        InsomniaPriority::ImmediateLive
    } else {
        match e.origin {
            EpisodeOrigin::Live => InsomniaPriority::Live,
            EpisodeOrigin::Import => InsomniaPriority::Import,
        }
    }
}
fn to_work(w: InsomniaWork) -> lower::InsomniaWork {
    lower::InsomniaWork {
        episode_id: lower::EpisodeId(w.episode_id.0),
        priority: w.priority.into(),
        state: w.state.into(),
        attempt_count: w.attempt_count,
        lease_owner: w.lease_owner,
        lease_token: w.lease_token.map(|t| lower::InsomniaLeaseToken(t.0)),
        lease_expires_ns: w.lease_expires_ns,
        retry_after_ns: w.retry_after_ns,
        last_error: w.last_error,
        updated_at_ns: w.updated_at_ns,
    }
}
fn from_work(w: lower::InsomniaWork) -> InsomniaWork {
    InsomniaWork {
        episode_id: crate::EpisodeId(w.episode_id.0),
        priority: w.priority.into(),
        state: w.state.into(),
        attempt_count: w.attempt_count,
        lease_owner: w.lease_owner,
        lease_token: w.lease_token.map(|t| InsomniaLeaseToken(t.0)),
        lease_expires_ns: w.lease_expires_ns,
        retry_after_ns: w.retry_after_ns,
        last_error: w.last_error,
        updated_at_ns: w.updated_at_ns,
    }
}
fn to_attempt(a: InsomniaAttempt) -> lower::InsomniaAttempt {
    lower::InsomniaAttempt {
        episode_id: lower::EpisodeId(a.episode_id.0),
        attempt: a.attempt,
        state: a.state.into(),
        started_at_ns: a.started_at_ns,
        completed_at_ns: a.completed_at_ns,
        extractor_model: a.extractor_model,
        extractor_version: a.extractor_version,
        memory_ids: a
            .memory_ids
            .into_iter()
            .map(|x| lower::MemoryId(x.0))
            .collect(),
        external_memory_refs: a
            .external_memory_refs
            .into_iter()
            .map(|r| lower::ExternalMemoryRef {
                owner_id: r.owner_id,
                memory_id: lower::MemoryId(r.memory_id.0),
            })
            .collect(),
        rejected_count: a.rejected_count,
        error: a.error,
    }
}
fn from_attempt(a: lower::InsomniaAttempt) -> InsomniaAttempt {
    InsomniaAttempt {
        episode_id: crate::EpisodeId(a.episode_id.0),
        attempt: a.attempt,
        state: a.state.into(),
        started_at_ns: a.started_at_ns,
        completed_at_ns: a.completed_at_ns,
        extractor_model: a.extractor_model,
        extractor_version: a.extractor_version,
        memory_ids: a.memory_ids.into_iter().map(|x| MemoryId(x.0)).collect(),
        external_memory_refs: a
            .external_memory_refs
            .into_iter()
            .map(|r| crate::MemoryRef {
                owner_id: r.owner_id,
                memory_id: MemoryId(r.memory_id.0),
            })
            .collect(),
        rejected_count: a.rejected_count,
        error: a.error,
    }
}
impl From<InsomniaPriority> for lower::InsomniaPriority {
    fn from(x: InsomniaPriority) -> Self {
        match x {
            InsomniaPriority::ImmediateLive => Self::ImmediateLive,
            InsomniaPriority::Live => Self::Live,
            InsomniaPriority::Import => Self::Import,
        }
    }
}
impl From<lower::InsomniaPriority> for InsomniaPriority {
    fn from(x: lower::InsomniaPriority) -> Self {
        match x {
            lower::InsomniaPriority::ImmediateLive => Self::ImmediateLive,
            lower::InsomniaPriority::Live => Self::Live,
            lower::InsomniaPriority::Import => Self::Import,
        }
    }
}
impl From<InsomniaWorkState> for lower::InsomniaWorkState {
    fn from(x: InsomniaWorkState) -> Self {
        match x {
            InsomniaWorkState::Pending => Self::Pending,
            InsomniaWorkState::Processing => Self::Processing,
            InsomniaWorkState::Complete => Self::Complete,
            InsomniaWorkState::Failed => Self::Failed,
            InsomniaWorkState::Terminal => Self::Terminal,
        }
    }
}
impl From<lower::InsomniaWorkState> for InsomniaWorkState {
    fn from(x: lower::InsomniaWorkState) -> Self {
        match x {
            lower::InsomniaWorkState::Pending => Self::Pending,
            lower::InsomniaWorkState::Processing => Self::Processing,
            lower::InsomniaWorkState::Complete => Self::Complete,
            lower::InsomniaWorkState::Failed => Self::Failed,
            lower::InsomniaWorkState::Terminal => Self::Terminal,
        }
    }
}
fn map_error(e: lower::InsomniaError) -> InsomniaError {
    match e {
        lower::InsomniaError::Container(e) => InsomniaError::Container(e),
        lower::InsomniaError::CorruptRecord(e) => InsomniaError::CorruptRecord(e),
        lower::InsomniaError::InvalidField(e) => InsomniaError::InvalidField(e),
        lower::InsomniaError::MissingFormat => InsomniaError::MissingFormat,
        lower::InsomniaError::ConflictingFormat => InsomniaError::ConflictingFormat,
        lower::InsomniaError::MissingEpisode => InsomniaError::MissingEpisode,
        lower::InsomniaError::ConflictingEpisode => InsomniaError::ConflictingEpisode,
        lower::InsomniaError::MissingWork => InsomniaError::MissingWork,
        lower::InsomniaError::InvalidLease => InsomniaError::InvalidLease,
        lower::InsomniaError::LeaseExpired => InsomniaError::LeaseExpired,
        lower::InsomniaError::InvalidTransition => InsomniaError::InvalidTransition,
    }
}
