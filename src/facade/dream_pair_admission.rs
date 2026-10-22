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
        self.dream_pairs.put(&mut self.container, left, right)
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
        self.dream_pairs.put(&mut self.container, left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.dream_pairs.records()
    }
}

pub(crate) fn validate_dream_pairs(
    pairs: &crate::dream_pair_history::DreamPairStore,
    memories: &crate::memory_store::MemoryStore,
) -> Result<(), MemoryError> {
    if pairs
        .referenced_memories()
        .all(|id| memories.contains_historical_memory(id))
    {
        Ok(())
    } else {
        Err(MemoryError::CorruptRecord(
            "Dream pair references a missing Memory",
        ))
    }
}
