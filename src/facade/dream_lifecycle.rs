//! Supported bindings for canonical REL Dream lifecycle execution.
use crate::{Cva, DreamLifecycleError, DreamLifecycleResult, MemoryId};

pub(crate) fn owner_facts(
    cva: &Cva,
    ids: impl IntoIterator<Item = MemoryId>,
    cohort_source: Option<MemoryId>,
) -> crate::dream_canonical::DreamOwnerFacts {
    crate::public_dream_operations::native_dream_owner_facts(cva, ids, cohort_source)
}

impl Cva {
    pub fn reconcile_dream_lifecycle(
        &mut self,
        source_id: MemoryId,
    ) -> Result<DreamLifecycleResult, DreamLifecycleError> {
        self.native_reconcile_dream_lifecycle(source_id)
    }
}
