use crate::{DreamLifecycleError, DreamLifecycleResult, MemoryId, Phylactery};

impl Phylactery {
    /// Public adapter for the lower PHY lifecycle owner operation.
    pub fn reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        let state = &mut self.owner;
        let graph = &state.graph;
        let mut memories =
            crate::memory_store::MemoryOwnerWriter::new(&mut state.memories, &mut state.container);
        crate::dream_owner_publisher::reconcile_phy_lifecycle(&mut memories, graph, source_id)
    }
}
