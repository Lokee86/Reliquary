use super::{InsomniaStore, clear_lease};
use crate::{
    Archive, Episode, EpisodeBoundary, EpisodeOrigin, InsomniaAttempt, InsomniaError,
    InsomniaPriority, InsomniaWork, InsomniaWorkState,
};

impl InsomniaStore {
    pub(crate) fn rebuild_schedule(&mut self, archive: &Archive) -> Result<(), InsomniaError> {
        let persisted = std::mem::take(&mut self.work);
        self.state_counts = [0; 5];
        for episode in archive.historical_episodes() {
            let priority = priority_for_episode(&episode);
            let work = completion_work(&self.attempts, &episode, priority)
                .or_else(|| persisted_final_work(&persisted, &episode, priority))
                .unwrap_or_else(|| pending_work(&episode, priority));
            self.insert_work(work);
        }
        self.scheduler.rebuild(archive, &self.work)
    }
}

fn persisted_final_work(
    persisted: &std::collections::HashMap<crate::EpisodeId, InsomniaWork>,
    episode: &Episode,
    priority: InsomniaPriority,
) -> Option<InsomniaWork> {
    let mut work = persisted
        .get(&episode.id)
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

fn priority_for_episode(episode: &Episode) -> InsomniaPriority {
    if matches!(
        episode.boundary,
        EpisodeBoundary::CreateMemory | EpisodeBoundary::Explicit
    ) {
        InsomniaPriority::ImmediateLive
    } else {
        match episode.origin {
            EpisodeOrigin::Live => InsomniaPriority::Live,
            EpisodeOrigin::Import => InsomniaPriority::Import,
        }
    }
}

fn pending_work(episode: &Episode, priority: InsomniaPriority) -> InsomniaWork {
    InsomniaWork {
        episode_id: episode.id,
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
    episode: &Episode,
    priority: InsomniaPriority,
) -> Option<InsomniaWork> {
    let attempt = attempts.iter().rev().find(|attempt| {
        attempt.episode_id == episode.id && attempt.state == InsomniaWorkState::Complete
    })?;
    Some(InsomniaWork {
        episode_id: episode.id,
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
