use crate::{Archive, EpisodeId, InsomniaError, InsomniaPriority, InsomniaWork, InsomniaWorkState};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScheduleKey {
    priority: InsomniaPriority,
    source_through_ns: i64,
    conversation_id: String,
    start_node_id: String,
    episode_id: EpisodeId,
}

impl Ord for ScheduleKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.source_through_ns.cmp(&other.source_through_ns))
            .then_with(|| self.conversation_id.cmp(&other.conversation_id))
            .then_with(|| self.start_node_id.cmp(&other.start_node_id))
            .then_with(|| self.episode_id.0.cmp(&other.episode_id.0))
    }
}

impl PartialOrd for ScheduleKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ScheduleKey {
    fn new(archive: &Archive, work: &InsomniaWork) -> Result<Self, InsomniaError> {
        let episode = archive
            .episode(work.episode_id)
            .ok_or(InsomniaError::MissingEpisode)?;
        Ok(Self {
            priority: work.priority,
            source_through_ns: episode.source_through_ns,
            conversation_id: episode.conversation_id.clone(),
            start_node_id: episode.start_node_id.clone(),
            episode_id: work.episode_id,
        })
    }

    pub(super) fn episode_id(&self) -> EpisodeId {
        self.episode_id
    }
}

#[derive(Default)]
pub(super) struct Scheduler {
    keys: HashMap<EpisodeId, ScheduleKey>,
    ready: BTreeSet<ScheduleKey>,
    retry_at: BTreeMap<i64, BTreeSet<ScheduleKey>>,
    lease_expires: BTreeMap<i64, BTreeSet<ScheduleKey>>,
}

impl Scheduler {
    pub(super) fn rebuild(
        &mut self,
        archive: &Archive,
        work: &HashMap<EpisodeId, InsomniaWork>,
    ) -> Result<(), InsomniaError> {
        self.keys.clear();
        self.ready.clear();
        self.retry_at.clear();
        self.lease_expires.clear();
        for item in work.values() {
            if archive.episode(item.episode_id).is_none() {
                continue;
            }
            let key = ScheduleKey::new(archive, item)?;
            self.keys.insert(item.episode_id, key.clone());
            self.index(item, key);
        }
        Ok(())
    }

    pub(super) fn register(
        &mut self,
        archive: &Archive,
        work: &InsomniaWork,
    ) -> Result<(), InsomniaError> {
        let key = ScheduleKey::new(archive, work)?;
        self.keys.insert(work.episode_id, key.clone());
        self.index(work, key);
        Ok(())
    }

    pub(super) fn replace(&mut self, old: &InsomniaWork, new: &InsomniaWork) {
        self.unindex(old);
        if let Some(key) = self.keys.get(&new.episode_id).cloned() {
            self.index(new, key);
        }
    }

    pub(super) fn next_ready(&mut self, now_ns: i64) -> Option<ScheduleKey> {
        self.promote_due(now_ns);
        self.ready.first().cloned()
    }

    fn promote_due(&mut self, now_ns: i64) {
        for key in take_due(&mut self.retry_at, now_ns) {
            self.ready.insert(key);
        }
        for key in take_due(&mut self.lease_expires, now_ns) {
            self.ready.insert(key);
        }
    }

    fn index(&mut self, work: &InsomniaWork, key: ScheduleKey) {
        match work.state {
            InsomniaWorkState::Pending => {
                self.ready.insert(key);
            }
            InsomniaWorkState::Failed => {
                if let Some(at) = work.retry_after_ns {
                    self.retry_at.entry(at).or_default().insert(key);
                }
            }
            InsomniaWorkState::Processing => {
                if let Some(at) = work.lease_expires_ns {
                    self.lease_expires.entry(at).or_default().insert(key);
                }
            }
            InsomniaWorkState::Complete | InsomniaWorkState::Terminal => {}
        }
    }

    fn unindex(&mut self, work: &InsomniaWork) {
        let Some(key) = self.keys.get(&work.episode_id).cloned() else {
            return;
        };
        self.ready.remove(&key);
        if let Some(at) = work.retry_after_ns {
            remove_timed(&mut self.retry_at, at, &key);
        }
        if let Some(at) = work.lease_expires_ns {
            remove_timed(&mut self.lease_expires, at, &key);
        }
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize, usize) {
        (
            self.ready.len(),
            self.retry_at.values().map(BTreeSet::len).sum(),
            self.lease_expires.values().map(BTreeSet::len).sum(),
        )
    }
}

fn take_due(index: &mut BTreeMap<i64, BTreeSet<ScheduleKey>>, now_ns: i64) -> Vec<ScheduleKey> {
    let times: Vec<_> = index.range(..=now_ns).map(|(at, _)| *at).collect();
    let mut due = Vec::new();
    for at in times {
        if let Some(items) = index.remove(&at) {
            due.extend(items);
        }
    }
    due
}

fn remove_timed(index: &mut BTreeMap<i64, BTreeSet<ScheduleKey>>, at: i64, key: &ScheduleKey) {
    let remove_bucket = index.get_mut(&at).is_some_and(|items| {
        items.remove(key);
        items.is_empty()
    });
    if remove_bucket {
        index.remove(&at);
    }
}
