use crate::EpisodeId;

/// Insomnia-owned read view. Import sees accepted state queries, not the
/// queue/store representation.
pub(crate) struct ImportReadiness<'a> {
    store: &'a super::store::InsomniaStore,
}

impl<'a> ImportReadiness<'a> {
    pub(crate) fn new(store: &'a super::store::InsomniaStore) -> Self {
        Self { store }
    }

    pub(crate) fn state(&self, episode_id: EpisodeId) -> Option<crate::InsomniaWorkState> {
        self.store.work(episode_id).map(|work| work.state)
    }
}
