use super::{InsomniaEpisodeSource, InsomniaStore, clear_lease};
use crate::{InsomniaAttempt, InsomniaError, InsomniaPriority, InsomniaWork, InsomniaWorkState};

impl InsomniaStore {
    pub(crate) fn rebuild_schedule(
        &mut self,
        sources: &[InsomniaEpisodeSource],
    ) -> Result<(), InsomniaError> {
        let persisted = std::mem::take(&mut self.work);
        self.state_counts = [0; 5];
        for episode in sources {
            let priority = episode.priority;
            let work = completion_work(&self.attempts, episode, priority)
                .or_else(|| persisted_final_work(&persisted, episode, priority))
                .unwrap_or_else(|| pending_work(episode, priority));
            self.insert_work(work);
        }
        self.scheduler.rebuild(sources, &self.work)
    }
}

fn persisted_final_work(
    persisted: &std::collections::HashMap<crate::EpisodeId, InsomniaWork>,
    episode: &InsomniaEpisodeSource,
    priority: InsomniaPriority,
) -> Option<InsomniaWork> {
    let mut work = persisted
        .get(&episode.episode_id)
        .filter(|work| {
            matches!(
                work.state,
                InsomniaWorkState::Complete | InsomniaWorkState::Terminal
            )
        })?
        .clone();
    work.priority = priority;
    clear_lease(&mut work);
    work.retry_after_ns = None;
    Some(work)
}

fn pending_work(episode: &InsomniaEpisodeSource, priority: InsomniaPriority) -> InsomniaWork {
    InsomniaWork {
        episode_id: episode.episode_id,
        priority,
        state: InsomniaWorkState::Pending,
        attempt_count: 0,
        lease_owner: None,
        lease_token: None,
        lease_expires_ns: None,
        retry_after_ns: None,
        last_error: None,
        updated_at_ns: episode.finalized_at_ns,
    }
}

fn completion_work(
    attempts: &[InsomniaAttempt],
    episode: &InsomniaEpisodeSource,
    priority: InsomniaPriority,
) -> Option<InsomniaWork> {
    let attempt = attempts.iter().rev().find(|attempt| {
        attempt.episode_id == episode.episode_id && attempt.state == InsomniaWorkState::Complete
    })?;
    Some(InsomniaWork {
        episode_id: episode.episode_id,
        priority,
        state: InsomniaWorkState::Complete,
        attempt_count: attempt.attempt,
        lease_owner: None,
        lease_token: None,
        lease_expires_ns: None,
        retry_after_ns: None,
        last_error: None,
        updated_at_ns: attempt.completed_at_ns,
    })
}
