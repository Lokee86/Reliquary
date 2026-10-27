use crate::{AcceptedEpisodeOrder, EpisodeId, InsomniaPriority, InsomniaWork, InsomniaWorkState};
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[derive(Clone, Debug, Eq, PartialEq)]
struct Key {
    priority: InsomniaPriority,
    source: i64,
    conversation: String,
    start: String,
    id: EpisodeId,
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&o.priority)
            .then(self.source.cmp(&o.source))
            .then(self.conversation.cmp(&o.conversation))
            .then(self.start.cmp(&o.start))
            .then(self.id.cmp(&o.id))
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Key {
    fn new(e: &AcceptedEpisodeOrder, p: InsomniaPriority) -> Self {
        Self {
            priority: p,
            source: e.source_through_ns,
            conversation: e.conversation_id.clone(),
            start: e.start_node_id.clone(),
            id: e.episode_id,
        }
    }
}
#[derive(Default)]
pub struct Scheduler {
    keys: HashMap<EpisodeId, Key>,
    ready: BTreeSet<Key>,
    retry: BTreeMap<i64, BTreeSet<Key>>,
    expiry: BTreeMap<i64, BTreeSet<Key>>,
}
impl Scheduler {
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.ready.len(),
            self.retry.values().map(BTreeSet::len).sum(),
            self.expiry.values().map(BTreeSet::len).sum(),
        )
    }

    pub fn rebuild(
        &mut self,
        orders: &[AcceptedEpisodeOrder],
        work: &HashMap<EpisodeId, InsomniaWork>,
    ) {
        *self = Self::default();
        let by: HashMap<_, _> = orders.iter().map(|e| (e.episode_id, e)).collect();
        for w in work.values() {
            if let Some(e) = by.get(&w.episode_id) {
                let k = Key::new(e, w.priority);
                self.keys.insert(w.episode_id, k.clone());
                self.index(w, &k)
            }
        }
    }
    pub fn register(&mut self, e: &AcceptedEpisodeOrder, w: &InsomniaWork) {
        let k = Key::new(e, w.priority);
        self.keys.insert(w.episode_id, k.clone());
        self.index(w, &k)
    }
    pub fn replace(&mut self, o: &InsomniaWork, n: &InsomniaWork) {
        self.unindex(o);
        if let Some(k) = self.keys.get(&n.episode_id).cloned() {
            self.index(n, &k)
        }
    }
    pub fn next_ready(&mut self, now: i64) -> Option<EpisodeId> {
        for k in due(&mut self.retry, now)
            .into_iter()
            .chain(due(&mut self.expiry, now))
        {
            self.ready.insert(k);
        }
        self.ready.first().map(|k| k.id)
    }
    fn index(&mut self, w: &InsomniaWork, k: &Key) {
        match w.state {
            InsomniaWorkState::Pending => {
                self.ready.insert(k.clone());
            }
            InsomniaWorkState::Failed => {
                if let Some(t) = w.retry_after_ns {
                    self.retry.entry(t).or_default().insert(k.clone());
                }
            }
            InsomniaWorkState::Processing => {
                if let Some(t) = w.lease_expires_ns {
                    self.expiry.entry(t).or_default().insert(k.clone());
                }
            }
            _ => {}
        }
    }
    fn unindex(&mut self, w: &InsomniaWork) {
        if let Some(k) = self.keys.get(&w.episode_id).cloned() {
            self.ready.remove(&k);
            if let Some(t) = w.retry_after_ns {
                remove(&mut self.retry, t, &k)
            }
            if let Some(t) = w.lease_expires_ns {
                remove(&mut self.expiry, t, &k)
            }
        }
    }
}
fn due(m: &mut BTreeMap<i64, BTreeSet<Key>>, t: i64) -> Vec<Key> {
    let ks: Vec<_> = m.range(..=t).map(|(k, _)| *k).collect();
    ks.into_iter()
        .flat_map(|k| m.remove(&k).unwrap_or_default())
        .collect()
}
fn remove(m: &mut BTreeMap<i64, BTreeSet<Key>>, t: i64, k: &Key) {
    if let Some(s) = m.get_mut(&t) {
        s.remove(k);
        if s.is_empty() {
            m.remove(&t);
        }
    }
}
