use super::codec::encode_format;
use crate::{
    Archive, Container, EpisodeId, InsomniaAttempt, InsomniaError, InsomniaLeaseToken,
    InsomniaPriority, InsomniaStats, InsomniaWork, InsomniaWorkState,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[path = "store/rebuild.rs"]
mod rebuild;
#[path = "store/scheduler.rs"]
mod scheduler;
#[path = "store/transitions.rs"]
mod transitions;

use scheduler::Scheduler;

pub(crate) struct InsomniaStore {
    work: HashMap<EpisodeId, InsomniaWork>,
    attempts: Vec<InsomniaAttempt>,
    scheduler: Scheduler,
    state_counts: [usize; 5],
}

impl InsomniaStore {
    pub(crate) fn empty() -> Self {
        Self {
            work: HashMap::new(),
            attempts: Vec::new(),
            scheduler: Scheduler::default(),
            state_counts: [0; 5],
        }
    }

    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), InsomniaError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn queue(
        &mut self,
        _container: &mut Container,
        archive: &Archive,
        episode_id: EpisodeId,
        priority: InsomniaPriority,
        now_ns: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        if let Some(existing) = self.work.get(&episode_id) {
            return Ok((existing.clone(), false));
        }
        let work = InsomniaWork {
            episode_id,
            priority,
            state: InsomniaWorkState::Pending,
            attempt_count: 0,
            lease_owner: None,
            lease_token: None,
            lease_expires_ns: None,
            retry_after_ns: None,
            last_error: None,
            updated_at_ns: now_ns,
        };
        self.scheduler.register(archive, &work)?;
        self.insert_work(work.clone());
        Ok((work, true))
    }

    pub(crate) fn claim_next(
        &mut self,
        _container: &mut Container,
        worker_id: &str,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        if worker_id.trim().is_empty() || lease_duration_ns <= 0 {
            return Err(InsomniaError::InvalidField("worker lease"));
        }
        let Some(key) = self.scheduler.next_ready(now_ns) else {
            return Ok(None);
        };
        let episode_id = key.episode_id();
        let current = self
            .work
            .get(&episode_id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        let attempt_count = current.attempt_count.saturating_add(1);
        let token = lease_token(episode_id, worker_id, attempt_count, now_ns);
        let mut claimed = current;
        claimed.state = InsomniaWorkState::Processing;
        claimed.attempt_count = attempt_count;
        claimed.lease_owner = Some(worker_id.trim().to_owned());
        claimed.lease_token = Some(token);
        claimed.lease_expires_ns = Some(now_ns.saturating_add(lease_duration_ns));
        claimed.retry_after_ns = None;
        claimed.last_error = None;
        claimed.updated_at_ns = now_ns;
        self.replace_work(claimed.clone())?;
        Ok(Some(claimed))
    }

    pub(crate) fn renew(
        &mut self,
        _container: &mut Container,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        now_ns: i64,
        lease_duration_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        if lease_duration_ns <= 0 {
            return Err(InsomniaError::InvalidField("lease duration"));
        }
        let mut work = self.active_claim(episode_id, token, now_ns)?;
        work.lease_expires_ns = Some(now_ns.saturating_add(lease_duration_ns));
        work.updated_at_ns = now_ns;
        self.replace_work(work.clone())?;
        Ok(work)
    }

    pub(crate) fn work(&self, episode_id: EpisodeId) -> Option<&InsomniaWork> {
        self.work.get(&episode_id)
    }

    pub(crate) fn attempts(&self, episode_id: EpisodeId) -> Vec<InsomniaAttempt> {
        self.attempts
            .iter()
            .filter(|attempt| attempt.episode_id == episode_id)
            .cloned()
            .collect()
    }

    pub(crate) fn stats(&self) -> InsomniaStats {
        InsomniaStats {
            total: self.work.len(),
            pending: self.state_counts[0],
            processing: self.state_counts[1],
            complete: self.state_counts[2],
            failed: self.state_counts[3],
            terminal: self.state_counts[4],
            attempts: self.attempts.len(),
        }
    }

    pub(crate) fn apply_work(&mut self, work: InsomniaWork) {
        self.insert_work(work);
    }

    pub(crate) fn apply_attempt(&mut self, attempt: InsomniaAttempt) {
        self.attempts.push(attempt);
    }

    pub(crate) fn apply_completion(
        &mut self,
        completion: &super::completion::InsomniaCompletion,
    ) -> Result<(), InsomniaError> {
        if completion.attempt == 0 {
            return Err(InsomniaError::InvalidTransition);
        }
        if let Some(mut work) = self.work.get(&completion.episode_id).cloned() {
            if completion.attempt < work.attempt_count {
                return Err(InsomniaError::InvalidTransition);
            }
            let old = work.clone();
            work.state = InsomniaWorkState::Complete;
            work.attempt_count = completion.attempt;
            clear_lease(&mut work);
            work.retry_after_ns = None;
            work.last_error = None;
            work.updated_at_ns = completion.completed_at_ns;
            self.scheduler.replace(&old, &work);
            self.insert_work(work);
        }
        self.attempts
            .retain(|attempt| attempt.episode_id != completion.episode_id);
        self.attempts.push(InsomniaAttempt {
            episode_id: completion.episode_id,
            attempt: completion.attempt,
            state: InsomniaWorkState::Complete,
            started_at_ns: completion.started_at_ns,
            completed_at_ns: completion.completed_at_ns,
            extractor_model: completion.extractor_model.clone(),
            extractor_version: completion.extractor_version.clone(),
            memory_ids: completion.memory_ids.clone(),
            external_memory_refs: completion.external_memory_refs.clone(),
            rejected_count: completion.rejected_count,
            error: None,
        });
        Ok(())
    }

    pub(crate) fn validate(
        &self,
        archive: &Archive,
        memories: &crate::memory_store::MemoryStore,
    ) -> Result<(), InsomniaError> {
        if !self
            .work
            .keys()
            .all(|episode_id| archive.historical_episode(*episode_id).is_some())
            || !self
                .attempts
                .iter()
                .all(|attempt| archive.historical_episode(attempt.episode_id).is_some())
        {
            return Err(InsomniaError::MissingEpisode);
        }
        if self
            .attempts
            .iter()
            .flat_map(|attempt| attempt.memory_ids.iter())
            .all(|id| memories.contains_historical_memory(*id))
        {
            Ok(())
        } else {
            Err(InsomniaError::InvalidTransition)
        }
    }

    pub(super) fn active_claim(
        &self,
        episode_id: EpisodeId,
        token: InsomniaLeaseToken,
        now_ns: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        let work = self
            .work
            .get(&episode_id)
            .ok_or(InsomniaError::MissingWork)?;
        if work.state != InsomniaWorkState::Processing || work.lease_token != Some(token) {
            return Err(InsomniaError::InvalidLease);
        }
        if work
            .lease_expires_ns
            .is_none_or(|expires| expires <= now_ns)
        {
            return Err(InsomniaError::LeaseExpired);
        }
        Ok(work.clone())
    }

    pub(super) fn replace_work(&mut self, work: InsomniaWork) -> Result<(), InsomniaError> {
        let old = self
            .work
            .get(&work.episode_id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        self.scheduler.replace(&old, &work);
        self.insert_work(work);
        Ok(())
    }

    fn insert_work(&mut self, work: InsomniaWork) {
        if let Some(old) = self.work.insert(work.episode_id, work.clone()) {
            self.state_counts[state_slot(old.state)] -= 1;
        }
        self.state_counts[state_slot(work.state)] += 1;
    }

    #[cfg(test)]
    pub(crate) fn scheduler_counts(&self) -> (usize, usize, usize) {
        self.scheduler.counts()
    }
}

pub(super) fn clear_lease(work: &mut InsomniaWork) {
    work.lease_owner = None;
    work.lease_token = None;
    work.lease_expires_ns = None;
}

fn state_slot(state: InsomniaWorkState) -> usize {
    match state {
        InsomniaWorkState::Pending => 0,
        InsomniaWorkState::Processing => 1,
        InsomniaWorkState::Complete => 2,
        InsomniaWorkState::Failed => 3,
        InsomniaWorkState::Terminal => 4,
    }
}

fn lease_token(
    episode_id: EpisodeId,
    worker_id: &str,
    attempt: u32,
    now_ns: i64,
) -> InsomniaLeaseToken {
    let mut hash = Sha256::new();
    hash.update(b"continuity-insomnia-lease\0");
    hash.update(episode_id.0);
    hash.update((worker_id.len() as u64).to_le_bytes());
    hash.update(worker_id.as_bytes());
    hash.update(attempt.to_le_bytes());
    hash.update(now_ns.to_le_bytes());
    InsomniaLeaseToken(hash.finalize().into())
}
