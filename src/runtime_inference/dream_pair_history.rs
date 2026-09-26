use crate::memory_store::MemoryStore;
use crate::{Container, Cva, MemoryError, MemoryId, Phylactery};
use std::collections::HashSet;

const DREAM_PAIR_MAGIC: [u8; 8] = *b"CVADRP01";

#[derive(Default)]
pub(crate) struct DreamPairStore {
    pairs: HashSet<(MemoryId, MemoryId)>,
}

impl DreamPairStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        if payload.len() < 8 || payload[..8] != DREAM_PAIR_MAGIC {
            return Ok(());
        }
        if payload.len() != 72 {
            return Err(MemoryError::CorruptRecord("invalid Dream pair record"));
        }
        let left = MemoryId(payload[8..40].try_into().unwrap());
        let right = MemoryId(payload[40..72].try_into().unwrap());
        if left == right {
            return Err(MemoryError::CorruptRecord("Dream pair self-reference"));
        }
        self.pairs.insert(canonical_pair(left, right));
        Ok(())
    }

    pub(crate) fn contains(&self, left: MemoryId, right: MemoryId) -> bool {
        left != right && self.pairs.contains(&canonical_pair(left, right))
    }

    pub(crate) fn records(&self) -> Vec<(MemoryId, MemoryId)> {
        let mut records: Vec<_> = self.pairs.iter().copied().collect();
        records.sort_by_key(|(left, right)| (left.0, right.0));
        records
    }

    pub(crate) fn validate(&self, memories: &MemoryStore) -> Result<(), MemoryError> {
        if self.pairs.iter().all(|(left, right)| {
            memories.contains_memory(*left) && memories.contains_memory(*right)
        }) {
            Ok(())
        } else {
            Err(MemoryError::CorruptRecord(
                "Dream pair references a missing Memory",
            ))
        }
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, MemoryError> {
        if left == right {
            return Err(MemoryError::InvalidField("Dream pair"));
        }
        let pair = canonical_pair(left, right);
        if self.pairs.contains(&pair) {
            return Ok(false);
        }
        let mut payload = Vec::with_capacity(72);
        payload.extend_from_slice(&DREAM_PAIR_MAGIC);
        payload.extend_from_slice(&pair.0.0);
        payload.extend_from_slice(&pair.1.0);
        container.append(&payload)?;
        self.pairs.insert(pair);
        Ok(true)
    }
}

fn canonical_pair(left: MemoryId, right: MemoryId) -> (MemoryId, MemoryId) {
    if left.0 <= right.0 {
        (left, right)
    } else {
        (right, left)
    }
}

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
