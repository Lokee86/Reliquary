//! One physical Dream carrier: exactly one positive Graph effect and its
//! evaluated-pair receipt. The owner validates its Memory and Graph cut.
use super::DreamMemoryCut;
use reliquary_graph::{NodeRef, Relation, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::MemoryId;

const MAGIC: &[u8; 8] = b"RELDRG01";
const PREFIX: &[u8; 6] = b"RELDRG";
const LEN: usize = 8 + 16 + 8 + 2 * (32 + 8 + 32) + 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamRelationCut {
    pub owner_id: [u8; 16],
    pub graph_version: u64,
    pub source: DreamMemoryCut,
    pub candidate: DreamMemoryCut,
    pub change: RelationChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GroupedDreamRelation(pub DreamRelationCut);

impl GroupedDreamRelation {
    pub(crate) fn new(cut: DreamRelationCut) -> Result<Self, String> {
        if cut.source.id == cut.candidate.id
            || cut.source.revision == 0
            || cut.candidate.revision == 0
            || cut.graph_version == u64::MAX
            || cut.change.origin != RelationOrigin::Dream
            || !cut.change.active
        {
            return Err("unsupported grouped Dream cut".into());
        }
        let endpoints = (cut.change.source, cut.change.target);
        let forward = (
            NodeRef::Memory(cut.source.id.0),
            NodeRef::Memory(cut.candidate.id.0),
        );
        if endpoints != forward && endpoints != (forward.1, forward.0) {
            return Err("grouped Dream relation endpoints mismatch".into());
        }
        if !matches!(
            cut.change.kind,
            RelationKind::Topical
                | RelationKind::Factual
                | RelationKind::Causal
                | RelationKind::Recurrent
                | RelationKind::DuplicateOf
        ) {
            return Err("grouped Dream lifecycle relation unsupported".into());
        }
        Ok(Self(cut))
    }

    pub(crate) fn pair(&self) -> (MemoryId, MemoryId) {
        (self.0.source.id, self.0.candidate.id)
    }

    pub(crate) fn relation(&self) -> Relation {
        let c = self.0;
        Relation {
            source: c.change.source,
            target: c.change.target,
            kind: c.change.kind,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: c.graph_version + 1,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let c = self.0;
        let mut out = Vec::with_capacity(LEN);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&c.owner_id);
        out.extend_from_slice(&c.graph_version.to_le_bytes());
        for observed in [c.source, c.candidate] {
            out.extend_from_slice(&observed.id.0);
            out.extend_from_slice(&observed.revision.to_le_bytes());
            out.extend_from_slice(&observed.body_id.0);
        }
        out.push(match c.change.kind {
            RelationKind::Topical => 1,
            RelationKind::Factual => 2,
            RelationKind::Causal => 3,
            RelationKind::Recurrent => 4,
            RelationKind::DuplicateOf => 5,
            _ => unreachable!(),
        });
        out.push(u8::from(
            c.change.source == NodeRef::Memory(c.candidate.id.0),
        ));
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Self>, String> {
        if !bytes.starts_with(PREFIX) {
            return Ok(None);
        }
        if bytes.len() != LEN || !bytes.starts_with(MAGIC) {
            return Err("invalid grouped Dream carrier".into());
        }
        let mut pos = 8;
        let owner_id = take::<16>(bytes, &mut pos);
        let graph_version = u64::from_le_bytes(take::<8>(bytes, &mut pos));
        let mut observed = Vec::with_capacity(2);
        for _ in 0..2 {
            observed.push(DreamMemoryCut {
                id: MemoryId(take::<32>(bytes, &mut pos)),
                revision: u64::from_le_bytes(take::<8>(bytes, &mut pos)),
                body_id: reliquary_memory_core::MemoryBodyId(take::<32>(bytes, &mut pos)),
            });
        }
        let kind = match bytes[pos] {
            1 => RelationKind::Topical,
            2 => RelationKind::Factual,
            3 => RelationKind::Causal,
            4 => RelationKind::Recurrent,
            5 => RelationKind::DuplicateOf,
            _ => return Err("invalid grouped Dream relation kind".into()),
        };
        pos += 1;
        let (source, candidate) = (observed[0], observed[1]);
        let (a, b) = match bytes[pos] {
            0 => (source.id, candidate.id),
            1 => (candidate.id, source.id),
            _ => return Err("invalid grouped Dream direction".into()),
        };
        Self::new(DreamRelationCut {
            owner_id,
            graph_version,
            source,
            candidate,
            change: RelationChange {
                source: NodeRef::Memory(a.0),
                target: NodeRef::Memory(b.0),
                kind,
                active: true,
                origin: RelationOrigin::Dream,
            },
        })
        .map(Some)
    }
}

fn take<const N: usize>(bytes: &[u8], pos: &mut usize) -> [u8; N] {
    let chunk = bytes[*pos..*pos + N]
        .try_into()
        .expect("validated frame length");
    *pos += N;
    chunk
}
