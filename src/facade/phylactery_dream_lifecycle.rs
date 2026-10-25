use crate::{DreamLifecycleError, DreamLifecycleResult, MemoryId, Phylactery};

impl Phylactery {
    /// Apply the mounted PHY Dream lifecycle operation.
    pub fn reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        self.native_reconcile_dream_lifecycle(source_id)
    }
}
