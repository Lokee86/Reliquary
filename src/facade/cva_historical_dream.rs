use crate::{Cva, CvaError, InitialDreamPass, MemoryId};
use std::collections::BTreeMap;
impl Cva {
    /// Returns retained completed first-cycle evidence without changing scores or
    /// resident journal state. Missing entries (including transformed markers)
    /// must not be interpreted as verified no-link outcomes.
    pub fn historical_completed_initial_dream_passes(
        &self,
    ) -> Result<BTreeMap<MemoryId, InitialDreamPass>, CvaError> {
        self.runtime_historical_completed_initial_dream_passes()
    }

    /// Uses Dream's existing representative authority on the pinned completed
    /// graph. Historical age is checked by the producer, not physical birth order.
    pub fn historical_dream_representatives(
        &mut self,
        source_id: MemoryId,
        roots: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<BTreeMap<MemoryId, MemoryId>, CvaError> {
        self.runtime_historical_dream_representatives(source_id, roots)
    }
}
