//! Admitting callers compose Memory liveness with Dream-owned pair state.
use crate::{Cva, MemoryError, MemoryId, Phylactery};

impl Cva {
    pub(crate) fn mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(left) || !self.memories.contains_memory(right) {
            return Err(MemoryError::MissingMemory);
        }
        self.owner
            .dream_pairs
            .put(&mut self.owner.container, left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.dream_pairs.records()
    }
}

impl Phylactery {
    pub(crate) fn mark_dream_pair_evaluated(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        if !self.memories.contains_memory(left) || !self.memories.contains_memory(right) {
            return Err(MemoryError::MissingMemory);
        }
        self.owner
            .dream_pairs
            .put(&mut self.owner.container, left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.dream_pairs.records()
    }
}
