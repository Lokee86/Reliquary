use crate::{
    AcceptedCompletionState, AcceptedEpisodeOrder, AcceptedEpisodeState, CompletionCheck,
    EpisodeId, InsomniaAttempt, InsomniaError, InsomniaLeaseToken, InsomniaPriority, InsomniaStats,
    InsomniaWork, InsomniaWorkState,
    codec::{self, InsomniaRecord},
};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
pub struct InsomniaStore {
    work: HashMap<EpisodeId, InsomniaWork>,
    attempts: Vec<InsomniaAttempt>,
    scheduler: super::scheduler::Scheduler,
    counts: [usize; 5],
}
impl InsomniaStore {
    pub fn empty() -> Self {
        Self {
            work: HashMap::new(),
            attempts: Vec::new(),
            scheduler: Default::default(),
            counts: [0; 5],
        }
    }
    pub fn initialize(&self, c: &mut Container) -> Result<(), InsomniaError> {
        c.append(&codec::encode_format())?;
        Ok(())
    }
    pub fn queue(
        &mut self,
        _c: &mut Container,
        e: AcceptedEpisodeOrder,
        p: InsomniaPriority,
        now: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        if let Some(w) = self.work.get(&e.episode_id) {
            return Ok((w.clone(), false));
        }
        let w = InsomniaWork {
            episode_id: e.episode_id,
            priority: p,
            state: InsomniaWorkState::Pending,
            attempt_count: 0,
            lease_owner: None,
            lease_token: None,
            lease_expires_ns: None,
            retry_after_ns: None,
            last_error: None,
            updated_at_ns: now,
        };
        self.scheduler.register(&e, &w);
        self.insert(w.clone());
        Ok((w, true))
    }
    pub fn claim_next(
        &mut self,
        _c: &mut Container,
        worker: &str,
        now: i64,
        duration: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        if worker.trim().is_empty() || duration <= 0 {
            return Err(InsomniaError::InvalidField("worker lease"));
        }
        let Some(id) = self.scheduler.next_ready(now) else {
            return Ok(None);
        };
        let mut w = self
            .work
            .get(&id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        w.attempt_count = w.attempt_count.saturating_add(1);
        w.state = InsomniaWorkState::Processing;
        w.lease_owner = Some(worker.trim().to_owned());
        w.lease_token = Some(token(id, worker, w.attempt_count, now));
        w.lease_expires_ns = Some(now.saturating_add(duration));
        w.retry_after_ns = None;
        w.last_error = None;
        w.updated_at_ns = now;
        self.replace(w.clone())?;
        Ok(Some(w))
    }
    pub fn renew(
        &mut self,
        _c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
        duration: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        if duration <= 0 {
            return Err(InsomniaError::InvalidField("lease duration"));
        }
        let mut w = self.active(id, t, now)?;
        w.lease_expires_ns = Some(now.saturating_add(duration));
        w.updated_at_ns = now;
        self.replace(w.clone())?;
        Ok(w)
    }
    pub fn fail(
        &mut self,
        _c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        _started: i64,
        failed: i64,
        retry: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        if reason.trim().is_empty() || retry <= failed {
            return Err(InsomniaError::InvalidField("retry failure"));
        }
        let mut w = self.active(id, t, failed)?;
        w.state = InsomniaWorkState::Failed;
        clear(&mut w);
        w.retry_after_ns = Some(retry);
        w.last_error = Some(reason);
        w.updated_at_ns = failed;
        self.replace(w.clone())?;
        Ok(w)
    }
    pub fn terminal(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        _started: i64,
        at: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        if reason.trim().is_empty() {
            return Err(InsomniaError::InvalidField("terminal reason"));
        }
        let mut w = self.active(id, t, at)?;
        w.state = InsomniaWorkState::Terminal;
        clear(&mut w);
        w.retry_after_ns = None;
        w.last_error = Some(reason);
        w.updated_at_ns = at;
        c.append(&codec::encode_work(&w)?)?;
        self.replace(w.clone())?;
        Ok(w)
    }
    pub fn retry_terminal(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        let mut w = self
            .work
            .get(&id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        if w.state != InsomniaWorkState::Terminal {
            return Err(InsomniaError::InvalidTransition);
        }
        w.state = InsomniaWorkState::Pending;
        w.attempt_count = 0;
        clear(&mut w);
        w.retry_after_ns = None;
        w.last_error = None;
        w.updated_at_ns = now;
        c.append(&codec::encode_work(&w)?)?;
        self.replace(w.clone())?;
        Ok(w)
    }
    pub fn work(&self, id: EpisodeId) -> Option<&InsomniaWork> {
        self.work.get(&id)
    }
    pub fn work_episode_ids(&self) -> impl Iterator<Item = EpisodeId> + '_ {
        self.work.keys().copied()
    }
    pub fn all_attempts(&self) -> impl Iterator<Item = &InsomniaAttempt> + '_ {
        self.attempts.iter()
    }
    pub fn attempts(&self, id: EpisodeId) -> Vec<InsomniaAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.episode_id == id)
            .cloned()
            .collect()
    }
    pub fn stats(&self) -> InsomniaStats {
        InsomniaStats {
            total: self.work.len(),
            pending: self.counts[0],
            processing: self.counts[1],
            complete: self.counts[2],
            failed: self.counts[3],
            terminal: self.counts[4],
            attempts: self.attempts.len(),
        }
    }
    /// Diagnostic scheduler bucket counts. These indexes are acceleration
    /// state; the authoritative work records remain in `work`.
    pub fn scheduler_counts(&self) -> (usize, usize, usize) {
        self.scheduler.counts()
    }
    pub fn apply_work(&mut self, w: InsomniaWork) {
        self.insert(w)
    }
    pub fn apply_attempt(&mut self, a: InsomniaAttempt) {
        self.attempts.push(a)
    }
    pub fn apply_completion(&mut self, c: &AcceptedCompletionState) -> Result<(), InsomniaError> {
        if self.check_completion(c)? == CompletionCheck::AlreadyAccepted {
            return Ok(());
        }
        if let Some(mut w) = self.work.get(&c.episode_id).cloned() {
            if c.attempt < w.attempt_count {
                return Err(InsomniaError::InvalidTransition);
            }
            let old = w.clone();
            w.state = InsomniaWorkState::Complete;
            w.attempt_count = c.attempt;
            clear(&mut w);
            w.retry_after_ns = None;
            w.last_error = None;
            w.updated_at_ns = c.completed_at_ns;
            self.scheduler.replace(&old, &w);
            self.insert(w)
        }
        self.attempts.retain(|a| a.episode_id != c.episode_id);
        self.attempts.push(InsomniaAttempt {
            episode_id: c.episode_id,
            attempt: c.attempt,
            state: InsomniaWorkState::Complete,
            started_at_ns: c.started_at_ns,
            completed_at_ns: c.completed_at_ns,
            extractor_model: c.extractor_model.clone(),
            extractor_version: c.extractor_version.clone(),
            memory_ids: c.memory_ids.clone(),
            external_memory_refs: c.external_memory_refs.clone(),
            rejected_count: c.rejected_count,
            error: None,
        });
        Ok(())
    }
    /// Checks receipt identity and attempt progression without changing store
    /// state. Call before appending a full higher-level completion frame.
    pub fn check_completion(
        &self,
        c: &AcceptedCompletionState,
    ) -> Result<CompletionCheck, InsomniaError> {
        if c.attempt == 0 {
            return Err(InsomniaError::InvalidTransition);
        }
        if self
            .work
            .get(&c.episode_id)
            .is_some_and(|w| c.attempt < w.attempt_count)
        {
            return Err(InsomniaError::InvalidTransition);
        }
        let same_attempt_receipts = self
            .attempts
            .iter()
            .filter(|a| a.episode_id == c.episode_id && a.attempt == c.attempt);
        let mut found_same_attempt = false;
        for previous in same_attempt_receipts {
            found_same_attempt = true;
            if !completion_matches(previous, c) {
                return Err(InsomniaError::InvalidTransition);
            }
        }
        if found_same_attempt {
            // A later work frame may have changed the projection after the
            // receipt. Re-applying the completion in that physical order must
            // still restore the same final work state as the baseline.
            return Ok(match self.work.get(&c.episode_id) {
                None => CompletionCheck::AlreadyAccepted,
                Some(w)
                    if w.state == InsomniaWorkState::Complete && w.attempt_count == c.attempt =>
                {
                    CompletionCheck::AlreadyAccepted
                }
                Some(_) => CompletionCheck::Apply,
            });
        }
        if self
            .attempts
            .iter()
            .filter(|a| a.episode_id == c.episode_id)
            .map(|a| a.attempt)
            .max()
            .is_some_and(|latest| c.attempt <= latest)
        {
            return Err(InsomniaError::InvalidTransition);
        }
        Ok(CompletionCheck::Apply)
    }
    pub fn replay_record(&mut self, b: &[u8]) -> Result<bool, InsomniaError> {
        match codec::decode_record(b)? {
            Some(InsomniaRecord::Work(w)) => {
                self.apply_work(w);
                Ok(true)
            }
            Some(InsomniaRecord::Attempt(a)) => {
                self.apply_attempt(a);
                Ok(true)
            }
            None => Ok(false),
        }
    }
    pub fn rebuild_schedule(&mut self, orders: &[AcceptedEpisodeOrder]) {
        self.scheduler.rebuild(orders, &self.work)
    }
    /// Rebuild work from authenticated Archive Episodes and already accepted
    /// completion attempts. Transient queue/lease states are intentionally
    /// returned to Pending on reopen, matching the root owner's recovery rule.
    pub fn rebuild_episodes(&mut self, episodes: &[AcceptedEpisodeState]) {
        let persisted = std::mem::take(&mut self.work);
        self.counts = [0; 5];
        let mut orders = Vec::with_capacity(episodes.len());
        for accepted in episodes {
            let id = accepted.order.episode_id;
            let work = self
                .attempts
                .iter()
                .rev()
                .find(|a| a.episode_id == id && a.state == InsomniaWorkState::Complete)
                .map(|a| InsomniaWork {
                    episode_id: id,
                    priority: accepted.priority,
                    state: InsomniaWorkState::Complete,
                    attempt_count: a.attempt,
                    lease_owner: None,
                    lease_token: None,
                    lease_expires_ns: None,
                    retry_after_ns: None,
                    last_error: None,
                    updated_at_ns: a.completed_at_ns,
                })
                .or_else(|| {
                    persisted
                        .get(&id)
                        .filter(|w| {
                            matches!(
                                w.state,
                                InsomniaWorkState::Complete | InsomniaWorkState::Terminal
                            )
                        })
                        .map(|w| {
                            let mut w = w.clone();
                            w.priority = accepted.priority;
                            clear(&mut w);
                            w.retry_after_ns = None;
                            w
                        })
                })
                .unwrap_or_else(|| InsomniaWork {
                    episode_id: id,
                    priority: accepted.priority,
                    state: InsomniaWorkState::Pending,
                    attempt_count: 0,
                    lease_owner: None,
                    lease_token: None,
                    lease_expires_ns: None,
                    retry_after_ns: None,
                    last_error: None,
                    updated_at_ns: accepted.finalized_at_ns,
                });
            self.insert(work);
            orders.push(accepted.order.clone());
        }
        self.scheduler.rebuild(&orders, &self.work);
    }
    pub fn next_ready(&mut self, now: i64) -> Option<EpisodeId> {
        self.scheduler.next_ready(now)
    }
    pub fn active_claim(
        &self,
        id: EpisodeId,
        token: InsomniaLeaseToken,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.active(id, token, now)
    }
    fn active(
        &self,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        let w = self.work.get(&id).ok_or(InsomniaError::MissingWork)?;
        if w.state != InsomniaWorkState::Processing || w.lease_token != Some(t) {
            return Err(InsomniaError::InvalidLease);
        }
        if w.lease_expires_ns.is_none_or(|x| x <= now) {
            return Err(InsomniaError::LeaseExpired);
        }
        Ok(w.clone())
    }
    fn replace(&mut self, w: InsomniaWork) -> Result<(), InsomniaError> {
        let old = self
            .work
            .get(&w.episode_id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        self.scheduler.replace(&old, &w);
        self.insert(w);
        Ok(())
    }
    fn insert(&mut self, w: InsomniaWork) {
        if let Some(old) = self.work.insert(w.episode_id, w.clone()) {
            self.counts[slot(old.state)] -= 1
        }
        self.counts[slot(w.state)] += 1
    }
}
fn completion_matches(a: &InsomniaAttempt, c: &AcceptedCompletionState) -> bool {
    a.state == InsomniaWorkState::Complete
        && a.error.is_none()
        && a.started_at_ns == c.started_at_ns
        && a.completed_at_ns == c.completed_at_ns
        && a.extractor_model == c.extractor_model
        && a.extractor_version == c.extractor_version
        && a.memory_ids == c.memory_ids
        && a.external_memory_refs == c.external_memory_refs
        && a.rejected_count == c.rejected_count
}
fn clear(w: &mut InsomniaWork) {
    w.lease_owner = None;
    w.lease_token = None;
    w.lease_expires_ns = None
}
fn slot(s: InsomniaWorkState) -> usize {
    match s {
        InsomniaWorkState::Pending => 0,
        InsomniaWorkState::Processing => 1,
        InsomniaWorkState::Complete => 2,
        InsomniaWorkState::Failed => 3,
        InsomniaWorkState::Terminal => 4,
    }
}
fn token(id: EpisodeId, worker: &str, attempt: u32, now: i64) -> InsomniaLeaseToken {
    let mut h = Sha256::new();
    h.update(b"continuity-insomnia-lease\0");
    h.update(id.0);
    h.update((worker.len() as u64).to_le_bytes());
    h.update(worker.as_bytes());
    h.update(attempt.to_le_bytes());
    h.update(now.to_le_bytes());
    InsomniaLeaseToken(h.finalize().into())
}
