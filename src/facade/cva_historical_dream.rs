use crate::{Cva, CvaError, InitialDreamPass, MemoryId};
use std::collections::BTreeMap;
impl Cva {
    /// Returns retained completed first-cycle evidence without changing scores or
    /// resident journal state. Missing entries (including transformed markers)
    /// must not be interpreted as verified no-link outcomes.
    pub fn historical_completed_initial_dream_passes(
        &self,
    ) -> Result<BTreeMap<MemoryId, InitialDreamPass>, CvaError> {
        if self.owner_uuid().is_none() {
            return Err(CvaError::Freshness(
                "historical Dream proof requires a durable REL owner".into(),
            ));
        }
        crate::owner_lifecycle::dream_replay::retained_completed_passes(
            &self.container,
            &self.archive,
            &self.memories,
        )
    }

    /// Uses Dream's existing representative authority on the pinned completed
    /// graph. Historical age is checked by the producer, not physical birth order.
    pub fn historical_dream_representatives(
        &mut self,
        source_id: MemoryId,
        roots: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<BTreeMap<MemoryId, MemoryId>, CvaError> {
        crate::history::dream_representative::historical_dream_representatives(
            &crate::owner_lifecycle::HistoryReadAccess::new(&self.container),
            &self.archive,
            &self.memories,
            &self.dream_freshness,
            &self.graph_relations(),
            source_id,
            roots,
        )
    }
}
