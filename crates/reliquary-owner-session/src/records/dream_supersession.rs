//! One recoverable Dream Supersedes relation, evaluated pair and Memory delta.
//! The record carries standard Memory delta bytes; Memory owns interpretation.
use crate::records::{DreamMemoryCut, DreamRelationCut};
use reliquary_graph::{NodeRef, Relation, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{GroupedMemorySupersession, MemoryId};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"RELDRS01";
const PREFIX: &[u8; 6] = b"RELDRS";
const HEADER: usize = 8 + 16 + 8 + 2 * 72 + 8 + 4;
const MAX_DELTA: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GroupedDreamSupersession {
    pub(crate) cut: DreamRelationCut,
    pub(crate) updated_at_ns: i64,
    pub(crate) delta: Vec<u8>,
}

impl GroupedDreamSupersession {
    pub(crate) fn new(
        cut: DreamRelationCut,
        updated_at_ns: i64,
        delta: Vec<u8>,
    ) -> Result<Self, String> {
        if cut.source.id == cut.candidate.id
            || cut.source.revision == 0
            || cut.candidate.revision == 0
            || cut.graph_version == u64::MAX
            || !cut.change.active
            || cut.change.origin != RelationOrigin::Dream
            || cut.change.kind != RelationKind::Supersedes
            || delta.len() < 8
            || delta.len() > MAX_DELTA
            || !delta.starts_with(b"RELMDLT1")
        {
            return Err("invalid grouped Dream supersession".into());
        }
        let left = NodeRef::Memory(cut.source.id.0);
        let right = NodeRef::Memory(cut.candidate.id.0);
        if (cut.change.source, cut.change.target) != (left, right)
            && (cut.change.source, cut.change.target) != (right, left)
        {
            return Err("Dream supersession references another Memory".into());
        }
        let NodeRef::Memory(older) = cut.change.target else {
            return Err("Dream supersession target is not a Memory".into());
        };
        let (id, _) = reliquary_memory_core::history_change_in_frame(&delta)
            .map_err(|error| error.to_string())?
            .ok_or("Dream supersession lacks a Memory delta")?;
        if id != MemoryId(older) {
            return Err("Dream supersession delta targets wrong Memory".into());
        }
        Ok(Self {
            cut,
            updated_at_ns,
            delta,
        })
    }

    pub(crate) fn memory_event(&self) -> GroupedMemorySupersession {
        let NodeRef::Memory(older) = self.cut.change.target else {
            unreachable!()
        };
        let older = MemoryId(older);
        let source = if self.cut.source.id == older {
            self.cut.source
        } else {
            self.cut.candidate
        };
        let NodeRef::Memory(newer) = self.cut.change.source else {
            unreachable!()
        };
        GroupedMemorySupersession {
            older,
            newer: MemoryId(newer),
            previous_revision: source.revision,
            updated_at_ns: self.updated_at_ns,
            mutation_id: Self::mutation_id(&self.cut),
            delta: self.delta.clone(),
        }
    }

    pub(crate) fn mutation_id(cut: &DreamRelationCut) -> String {
        let mut hash = Sha256::new();
        hash.update(b"reliquary:dream-supersession:v1\0");
        hash.update(cut.owner_id);
        hash.update(cut.graph_version.to_le_bytes());
        if let NodeRef::Memory(newer) = cut.change.source {
            hash.update(newer);
        }
        if let NodeRef::Memory(older) = cut.change.target {
            hash.update(older);
        }
        format!("dream-supersession:{:x}", hash.finalize())
    }

    pub(crate) fn pair(&self) -> (MemoryId, MemoryId) {
        (self.cut.source.id, self.cut.candidate.id)
    }
    pub(crate) fn relation(&self) -> Relation {
        Relation {
            source: self.cut.change.source,
            target: self.cut.change.target,
            kind: RelationKind::Supersedes,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: self.cut.graph_version + 1,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let c = self.cut;
        let mut out = Vec::with_capacity(HEADER + self.delta.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&c.owner_id);
        out.extend_from_slice(&c.graph_version.to_le_bytes());
        for observed in [c.source, c.candidate] {
            out.extend_from_slice(&observed.id.0);
            out.extend_from_slice(&observed.revision.to_le_bytes());
            out.extend_from_slice(&observed.body_id.0);
        }
        out.extend_from_slice(&self.updated_at_ns.to_le_bytes());
        out.extend_from_slice(&(self.delta.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.delta);
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Self>, String> {
        if !bytes.starts_with(PREFIX) {
            return Ok(None);
        }
        if bytes.len() < HEADER || !bytes.starts_with(MAGIC) {
            return Err("malformed Dream supersession carrier".into());
        }
        let mut at = 8;
        let owner_id = take::<16>(bytes, &mut at);
        let graph_version = u64::from_le_bytes(take::<8>(bytes, &mut at));
        let mut observed = Vec::with_capacity(2);
        for _ in 0..2 {
            observed.push(DreamMemoryCut {
                id: MemoryId(take::<32>(bytes, &mut at)),
                revision: u64::from_le_bytes(take::<8>(bytes, &mut at)),
                body_id: reliquary_memory_core::MemoryBodyId(take::<32>(bytes, &mut at)),
            });
        }
        let updated_at_ns = i64::from_le_bytes(take::<8>(bytes, &mut at));
        let len = u32::from_le_bytes(take::<4>(bytes, &mut at)) as usize;
        if len > MAX_DELTA || bytes.len() != HEADER + len {
            return Err("Dream supersession carrier has invalid delta size".into());
        }
        let source = observed[0];
        let candidate = observed[1];
        // The delta selects the older endpoint, rather than relying on
        // Memory order or classifier direction supplied by an external host.
        let delta = bytes[at..].to_vec();
        let (older, _) = reliquary_memory_core::history_change_in_frame(&delta)
            .map_err(|error| error.to_string())?
            .ok_or("missing supersession delta")?;
        let newer = if older == source.id {
            candidate.id
        } else if older == candidate.id {
            source.id
        } else {
            return Err("supersession delta is outside observed pair".into());
        };
        Self::new(
            DreamRelationCut {
                owner_id,
                graph_version,
                source,
                candidate,
                change: RelationChange {
                    source: NodeRef::Memory(newer.0),
                    target: NodeRef::Memory(older.0),
                    kind: RelationKind::Supersedes,
                    active: true,
                    origin: RelationOrigin::Dream,
                },
            },
            updated_at_ns,
            delta,
        )
        .map(Some)
    }
}

fn take<const N: usize>(bytes: &[u8], at: &mut usize) -> [u8; N] {
    let chunk = bytes[*at..*at + N]
        .try_into()
        .expect("checked grouped Dream carrier size");
    *at += N;
    chunk
}
