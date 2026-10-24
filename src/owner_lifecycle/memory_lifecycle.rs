//! Native Memory publication and derived Graph exclusion share one owner session.
use super::{PhylacteryOwner, ReliquaryOwner, TombstoneOutcome};
use crate::{MemoryError, MemoryId, SemanticNodeRef};

macro_rules! owner_memory_lifecycle {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn tombstone_memory_native(
                &mut self,
                id: MemoryId,
                expected_revision: u64,
            ) -> Result<TombstoneOutcome, MemoryError> {
                let outcome =
                    self.memories
                        .tombstone(&mut self.container, id, expected_revision)?;
                self.graph
                    .exclude_from_current(SemanticNodeRef::memory(id))
                    .map_err(|_| MemoryError::InvalidField("Memory graph projection"))?;
                Ok(outcome)
            }
        }
    };
}
owner_memory_lifecycle!(ReliquaryOwner);
owner_memory_lifecycle!(PhylacteryOwner);
