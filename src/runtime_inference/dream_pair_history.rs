use crate::memory_store::MemoryStore;
use crate::{Container, Cva, MemoryError, MemoryId, Phylactery};
use std::collections::{HashMap, HashSet};

#[path = "dream_pair_codec.rs"]
mod codec;
#[cfg(test)]
#[path = "dream_pair_history_tests.rs"]
mod tests;

// Temporary legacy decoder for offline conversion; retired after corpus promotion.
const LEGACY_PAIR_MAGIC: [u8; 8] = *b"CVADRP01";

#[derive(Default)]
pub(crate) struct DreamPairStore {
    pairs: HashSet<(MemoryId, MemoryId)>,
    memory_ids: Vec<MemoryId>,
    memory_references: HashMap<MemoryId, u32>,
}

impl DreamPairStore {
    pub(crate) fn is_compact_payload(payload: &[u8]) -> bool {
        payload.starts_with(codec::MEMORY_MAGIC) || payload.starts_with(codec::PAIR_MAGIC)
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), MemoryError> {
        if let Some(pair) = Self::decode_legacy_pair(payload)? {
            self.pairs.insert(pair);
            return Ok(());
        }
        match codec::decode(payload)? {
            Some(codec::Record::Memory(reference, id)) => self.admit_mapping(reference, id)?,
            Some(codec::Record::Pair(left, right)) => {
                let pair = self.resolve_pair(left, right)?;
                self.pairs.insert(pair);
            }
            None => {}
        }
        Ok(())
    }

    pub(crate) fn decode_legacy_pair(
        payload: &[u8],
    ) -> Result<Option<(MemoryId, MemoryId)>, MemoryError> {
        if !payload.starts_with(&LEGACY_PAIR_MAGIC) {
            return Ok(None);
        }
        if payload.len() != 72 {
            return Err(MemoryError::CorruptRecord("invalid Dream pair record"));
        }
        let left = MemoryId(payload[8..40].try_into().unwrap());
        let right = MemoryId(payload[40..72].try_into().unwrap());
        if left == right {
            return Err(MemoryError::CorruptRecord("Dream pair self-reference"));
        }
        Ok(Some(canonical_pair(left, right)))
    }

    fn admit_mapping(&mut self, reference: u32, id: MemoryId) -> Result<(), MemoryError> {
        if let Some(existing) = self.memory_ids.get(reference.saturating_sub(1) as usize) {
            return if *existing == id {
                Ok(())
            } else {
                Err(MemoryError::CorruptRecord(
                    "conflicting Dream pair Memory reference",
                ))
            };
        }
        if usize::try_from(reference).ok() != self.memory_ids.len().checked_add(1)
            || self.memory_references.contains_key(&id)
        {
            return Err(MemoryError::CorruptRecord(
                "noncanonical Dream pair Memory mapping",
            ));
        }
        self.memory_ids.push(id);
        self.memory_references.insert(id, reference);
        Ok(())
    }

    fn resolve_pair(&self, left: u32, right: u32) -> Result<(MemoryId, MemoryId), MemoryError> {
        let lookup = |reference: u32| {
            self.memory_ids
                .get(reference.saturating_sub(1) as usize)
                .copied()
                .ok_or(MemoryError::CorruptRecord(
                    "missing Dream pair Memory mapping",
                ))
        };
        let (left, right) = (lookup(left)?, lookup(right)?);
        if left.0 >= right.0 {
            return Err(MemoryError::CorruptRecord(
                "noncanonical Dream pair identity order",
            ));
        }
        Ok((left, right))
    }

    #[cfg(test)]
    pub(crate) fn current_pair(
        &self,
        payload: &[u8],
    ) -> Result<Option<(MemoryId, MemoryId)>, MemoryError> {
        match codec::decode(payload)? {
            Some(codec::Record::Pair(left, right)) => self.resolve_pair(left, right).map(Some),
            _ => Ok(None),
        }
    }

    fn persist_reference(
        &mut self,
        container: &mut Container,
        id: MemoryId,
    ) -> Result<u32, MemoryError> {
        if let Some(reference) = self.memory_references.get(&id) {
            return Ok(*reference);
        }
        let reference = u32::try_from(self.memory_ids.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(MemoryError::CorruptRecord(
                "Dream pair Memory reference capacity exceeded",
            ))?;
        container.append(&codec::encode_memory(reference, id))?;
        self.admit_mapping(reference, id)?;
        Ok(reference)
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
        if self
            .memory_ids
            .iter()
            .all(|id| memories.contains_historical_memory(*id))
            && self.pairs.iter().all(|(left, right)| {
                memories.contains_historical_memory(*left)
                    && memories.contains_historical_memory(*right)
            })
        {
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
        container.require_storage_format_writable()?;
        if left == right {
            return Err(MemoryError::InvalidField("Dream pair"));
        }
        let pair = canonical_pair(left, right);
        if self.pairs.contains(&pair) {
            return Ok(false);
        }
        let left = self.persist_reference(container, pair.0)?;
        let right = self.persist_reference(container, pair.1)?;
        container.append(&codec::encode_pair(left, right))?;
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
        self.container.require_storage_format_writable()?;
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
        self.container.require_storage_format_writable()?;
        if !self.memories.contains_memory(left) || !self.memories.contains_memory(right) {
            return Err(MemoryError::MissingMemory);
        }
        self.dream_pairs.put(&mut self.container, left, right)
    }

    pub(crate) fn dream_pair_records(&self) -> Vec<(MemoryId, MemoryId)> {
        self.dream_pairs.records()
    }
}
