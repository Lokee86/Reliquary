//! Compact owner-local references for evaluated unordered Dream pairs.
use super::*;
use crate::records::{
    dream_group::GroupedDreamRelation, dream_supersession::GroupedDreamSupersession,
};
use std::collections::{BTreeMap, BTreeSet};

const MEMORY: &[u8; 8] = b"RELDRM01";
const PAIR: &[u8; 8] = b"RELDRP02";

pub(crate) struct DreamPairs {
    owner: [u8; 16],
    identity: ContainerIdentity,
    ids: Vec<MemoryId>,
    refs: BTreeMap<MemoryId, u32>,
    pairs: BTreeSet<(MemoryId, MemoryId)>,
    grouped_receipts: BTreeMap<(MemoryId, MemoryId), GroupedDreamRelation>,
    batches: BTreeMap<[u8; 32], crate::records::dream_batch::DreamBatchRecord>,
    initial: BTreeMap<MemoryId, BTreeMap<MemoryId, (reliquary_graph::RelationKind, u64)>>,
    supersessions: BTreeMap<(MemoryId, MemoryId), GroupedDreamSupersession>,
}

impl DreamPairs {
    pub(crate) fn open(container: &Container) -> Result<Self, OwnerSessionError> {
        let mut state = Self {
            owner: container.owner_id(),
            identity: container.identity(),
            ids: vec![],
            refs: BTreeMap::new(),
            pairs: BTreeSet::new(),
            grouped_receipts: BTreeMap::new(),
            batches: BTreeMap::new(),
            initial: BTreeMap::new(),
            supersessions: BTreeMap::new(),
        };
        let mut memories = BTreeSet::new();
        for frame in container.read_frames()? {
            let envelope = reliquary_container::decode_publication(&frame.payload)?;
            let payload = envelope
                .as_ref()
                .map_or(frame.payload.as_slice(), |value| value.body);
            if let Some(batch) = crate::records::dream_batch::DreamBatchRecord::decode(payload)
                .map_err(OwnerSessionError::Record)?
            {
                if batch.owner != state.owner
                    || batch
                        .observed
                        .iter()
                        .any(|o| !memories.contains(&MemoryId(o.id)))
                {
                    return Err(invalid());
                }
                state.apply_batch(batch)?;
            } else if payload.starts_with(b"RELMCRT1") {
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
            } else if let Some(group) =
                GroupedDreamRelation::decode(payload).map_err(OwnerSessionError::Record)?
            {
                let (left, right) = group.pair();
                if !memories.contains(&left) || !memories.contains(&right) {
                    return Err(invalid());
                }
                state.initial.entry(group.0.source.id).or_default().insert(
                    group.0.candidate.id,
                    (group.relation().kind, group.relation().graph_version),
                );
                let pair = ordered(left, right);
                if !state.pairs.insert(pair) || state.grouped_receipts.insert(pair, group).is_some()
                {
                    return Err(invalid());
                }
            } else if let Some(group) =
                GroupedDreamSupersession::decode(payload).map_err(OwnerSessionError::Record)?
            {
                let (left, right) = group.pair();
                if !memories.contains(&left) || !memories.contains(&right) {
                    return Err(invalid());
                }
                let pair = ordered(left, right);
                if !state.pairs.insert(pair) || state.supersessions.insert(pair, group).is_some() {
                    return Err(invalid());
                }
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

    pub(crate) fn grouped_receipt(
        &self,
        left: MemoryId,
        right: MemoryId,
    ) -> Option<GroupedDreamRelation> {
        self.grouped_receipts.get(&ordered(left, right)).copied()
    }

    pub(crate) fn apply_grouped(
        &mut self,
        group: GroupedDreamRelation,
    ) -> Result<(), OwnerSessionError> {
        let (left, right) = group.pair();
        let pair = ordered(left, right);
        if !self.pairs.insert(pair) {
            return Err(invalid());
        }
        self.initial.entry(group.0.source.id).or_default().insert(
            group.0.candidate.id,
            (group.relation().kind, group.relation().graph_version),
        );
        self.grouped_receipts.insert(pair, group);
        Ok(())
    }

    pub(crate) fn supersession_receipt(
        &self,
        left: MemoryId,
        right: MemoryId,
    ) -> Option<&GroupedDreamSupersession> {
        self.supersessions.get(&ordered(left, right))
    }

    pub(crate) fn apply_supersession(
        &mut self,
        group: GroupedDreamSupersession,
    ) -> Result<(), OwnerSessionError> {
        let (left, right) = group.pair();
        let pair = ordered(left, right);
        if !self.pairs.insert(pair) {
            return Err(invalid());
        }
        self.supersessions.insert(pair, group);
        Ok(())
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

fn ordered(a: MemoryId, b: MemoryId) -> (MemoryId, MemoryId) {
    if a < b { (a, b) } else { (b, a) }
}

fn invalid() -> OwnerSessionError {
    OwnerSessionError::Record("invalid Dream pair physical prefix".into())
}

#[path = "dream_pair_batches.rs"]
mod batches;
