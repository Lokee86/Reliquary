//! Compact owner-local references for evaluated unordered Dream pairs.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const MEMORY: &[u8; 8] = b"RELDRM01";
const PAIR: &[u8; 8] = b"RELDRP02";

pub(crate) struct DreamPairs {
    owner: [u8; 16],
    identity: ContainerIdentity,
    ids: Vec<MemoryId>,
    refs: BTreeMap<MemoryId, u32>,
    pairs: BTreeSet<(MemoryId, MemoryId)>,
}

impl DreamPairs {
    pub(crate) fn open(container: &Container) -> Result<Self, OwnerSessionError> {
        let mut state = Self {
            owner: container.owner_id(),
            identity: container.identity(),
            ids: vec![],
            refs: BTreeMap::new(),
            pairs: BTreeSet::new(),
        };
        let mut memories = BTreeSet::new();
        for frame in container.read_frames()? {
            let envelope = reliquary_container::decode_publication(&frame.payload)?;
            let payload = envelope
                .as_ref()
                .map_or(frame.payload.as_slice(), |value| value.body);
            if payload.starts_with(b"RELMCRT1") {
                memories.insert(reliquary_memory_core::decode_grouped_creation(payload)?.id);
            } else if payload.starts_with(b"RELINSC7") {
                let completion = GroupedInsomniaCompletion::decode(payload)
                    .map_err(OwnerSessionError::Record)?;
                for record in completion.memory_records {
                    memories.insert(reliquary_memory_core::decode_grouped_creation(&record)?.id);
                }
            } else if payload.starts_with(MEMORY) {
                if payload.len() != 44 {
                    return Err(invalid());
                }
                let reference = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                let id = MemoryId(payload[12..44].try_into().unwrap());
                if !memories.contains(&id) || reference == 0 {
                    return Err(invalid());
                }
                if let Some(existing) = state.ids.get((reference - 1) as usize) {
                    if *existing != id {
                        return Err(invalid());
                    }
                } else if usize::try_from(reference).ok() == state.ids.len().checked_add(1)
                    && !state.refs.contains_key(&id)
                {
                    state.ids.push(id);
                    state.refs.insert(id, reference);
                } else {
                    return Err(invalid());
                }
            } else if payload.starts_with(PAIR) {
                if payload.len() != 16 {
                    return Err(invalid());
                }
                let left = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                let right = u32::from_le_bytes(payload[12..16].try_into().unwrap());
                let Some(left) = left
                    .checked_sub(1)
                    .and_then(|n| state.ids.get(n as usize))
                    .copied()
                else {
                    return Err(invalid());
                };
                let Some(right) = right
                    .checked_sub(1)
                    .and_then(|n| state.ids.get(n as usize))
                    .copied()
                else {
                    return Err(invalid());
                };
                if left >= right {
                    return Err(invalid());
                }
                state.pairs.insert((left, right));
            } else if payload.starts_with(b"RELDRM") || payload.starts_with(b"RELDRP") {
                return Err(invalid());
            }
        }
        Ok(state)
    }

    pub(crate) fn contains(&self, left: MemoryId, right: MemoryId) -> bool {
        left != right
            && self.pairs.contains(&if left < right {
                (left, right)
            } else {
                (right, left)
            })
    }

    pub(super) fn append(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, OwnerSessionError> {
        if container.owner_id() != self.owner || container.identity() != self.identity {
            return Err(OwnerSessionError::Record(
                "Dream pair owner mismatch".into(),
            ));
        }
        if left == right || !memories.contains_memory(left) || !memories.contains_memory(right) {
            return Err(OwnerSessionError::Record(
                "Dream pair requires two owner Memories".into(),
            ));
        }
        let pair = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if self.pairs.contains(&pair) {
            return Ok(false);
        }
        let left = self.reference(container, pair.0)?;
        let right = self.reference(container, pair.1)?;
        let mut payload = PAIR.to_vec();
        payload.extend_from_slice(&left.to_le_bytes());
        payload.extend_from_slice(&right.to_le_bytes());
        container.append(&payload)?;
        container.sync()?;
        self.pairs.insert(pair);
        Ok(true)
    }

    fn reference(
        &mut self,
        container: &mut Container,
        id: MemoryId,
    ) -> Result<u32, OwnerSessionError> {
        if let Some(reference) = self.refs.get(&id) {
            return Ok(*reference);
        }
        let reference = u32::try_from(self.ids.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(invalid)?;
        let mut payload = MEMORY.to_vec();
        payload.extend_from_slice(&reference.to_le_bytes());
        payload.extend_from_slice(&id.0);
        container.append(&payload)?;
        self.ids.push(id);
        self.refs.insert(id, reference);
        Ok(reference)
    }
}

fn invalid() -> OwnerSessionError {
    OwnerSessionError::Record("invalid Dream pair physical prefix".into())
}
