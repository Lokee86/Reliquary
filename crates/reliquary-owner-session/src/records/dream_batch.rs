//! One synced carrier for bounded Graph, lifecycle and evaluated-pair effects.
use crate::session::dream_batch_model::*;
use crate::{DreamMemoryCut, MemoryId};
use reliquary_graph::{RelationChange, RelationKind};
use reliquary_memory_core::{GroupedMemoryLifecycle, MemoryBodyId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
const MAGIC: &[u8; 8] = b"RELDRB01";
const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Observation {
    pub id: [u8; 32],
    pub revision: u64,
    pub body: [u8; 32],
}
impl Observation {
    pub fn cut(&self) -> DreamMemoryCut {
        DreamMemoryCut {
            id: MemoryId(self.id),
            revision: self.revision,
            body_id: MemoryBodyId(self.body),
        }
    }
}
impl From<DreamMemoryCut> for Observation {
    fn from(c: DreamMemoryCut) -> Self {
        Self {
            id: c.id.0,
            revision: c.revision,
            body: c.body_id.0,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pair {
    pub source: [u8; 32],
    pub candidate: [u8; 32],
    pub kind: u8,
    pub rejected: bool,
    pub direct: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lifecycle {
    pub id: [u8; 32],
    pub revision: u64,
    pub state: String,
    pub archived: bool,
    pub superseded_by: Option<[u8; 32]>,
    pub mutation: String,
    pub updated_at_ns: i64,
    pub delta: Vec<u8>,
}
impl Lifecycle {
    pub fn event(&self) -> GroupedMemoryLifecycle {
        GroupedMemoryLifecycle {
            id: MemoryId(self.id),
            previous_revision: self.revision,
            state: self.state.clone(),
            archived: self.archived,
            superseded_by: self.superseded_by.map(MemoryId),
            mutation_id: self.mutation.clone(),
            updated_at_ns: self.updated_at_ns,
            delta: self.delta.clone(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DreamBatchRecord {
    pub owner: [u8; 16],
    pub publication_id: [u8; 32],
    pub fingerprint: [u8; 32],
    pub graph_version: u64,
    pub memory_version: u64,
    pub observed: Vec<Observation>,
    pub pairs: Vec<Pair>,
    pub graph_records: Vec<Vec<u8>>,
    pub lifecycles: Vec<Lifecycle>,
    pub updated_at_ns: i64,
}
impl DreamBatchRecord {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Dream batch exceeds byte limit".into());
        }
        let mut out = MAGIC.to_vec();
        out.extend(bytes);
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Option<Self>, String> {
        if !bytes.starts_with(b"RELDRB") {
            return Ok(None);
        }
        if !bytes.starts_with(MAGIC) || bytes.len() > MAX_BYTES + 8 {
            return Err("invalid Dream batch carrier".into());
        }
        let result: Self = serde_json::from_slice(&bytes[8..]).map_err(|e| e.to_string())?;
        result.validate()?;
        Ok(Some(result))
    }
    pub fn request(&self) -> Result<DreamBatchCut, String> {
        Ok(DreamBatchCut {
            owner: self.owner,
            publication_id: self.publication_id,
            graph_version: self.graph_version,
            observed: self.observed.iter().map(Observation::cut).collect(),
            pairs: self
                .pairs
                .iter()
                .map(|p| {
                    Ok(DreamPairPublication {
                        source: MemoryId(p.source),
                        candidate: MemoryId(p.candidate),
                        kind: decode_kind(p.kind)?,
                        rejected: p.rejected,
                    })
                })
                .collect::<Result<_, String>>()?,
            changes: self
                .graph_records
                .iter()
                .map(|r| {
                    reliquary_graph::dream_relation_in_frame(r, self.owner)
                        .map(|r| RelationChange {
                            source: r.source,
                            target: r.target,
                            kind: r.kind,
                            active: r.active,
                            origin: r.origin,
                        })
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?,
            lifecycles: self
                .lifecycles
                .iter()
                .map(|l| DreamLifecyclePublication {
                    memory_id: MemoryId(l.id),
                    lifecycle_state: l.state.clone(),
                    archived: l.archived,
                    superseded_by: l.superseded_by.map(MemoryId),
                })
                .collect(),
            updated_at_ns: self.updated_at_ns,
        })
    }
}
pub(crate) fn kind(kind: Option<RelationKind>) -> Result<u8, String> {
    Ok(match kind {
        None => 0,
        Some(RelationKind::Topical) => 1,
        Some(RelationKind::Factual) => 2,
        Some(RelationKind::Causal) => 3,
        Some(RelationKind::Recurrent) => 4,
        Some(RelationKind::DuplicateOf) => 5,
        Some(RelationKind::Supersedes) => 6,
        _ => return Err("unsupported Dream batch relation".into()),
    })
}
pub(crate) fn decode_kind(kind: u8) -> Result<Option<RelationKind>, String> {
    Ok(match kind {
        0 => None,
        1 => Some(RelationKind::Topical),
        2 => Some(RelationKind::Factual),
        3 => Some(RelationKind::Causal),
        4 => Some(RelationKind::Recurrent),
        5 => Some(RelationKind::DuplicateOf),
        6 => Some(RelationKind::Supersedes),
        _ => return Err("invalid Dream batch relation".into()),
    })
}
pub(crate) fn fingerprint(cut: &DreamBatchCut) -> Result<[u8; 32], String> {
    use serde_json::json;
    let value = json!({"owner":cut.owner,"publication":cut.publication_id,"graph":cut.graph_version,"observed":cut.observed.iter().map(|c|json!([c.id.0,c.revision,c.body_id.0])).collect::<Vec<_>>(),
        "pairs":cut.pairs.iter().map(|p|Ok(json!([p.source.0,p.candidate.0,kind(p.kind)?,p.rejected]))).collect::<Result<Vec<_>,String>>()?,
        "changes":cut.changes.iter().map(|c|json!([format!("{:?}",c.source),format!("{:?}",c.target),format!("{:?}",c.kind),c.active,format!("{:?}",c.origin)])).collect::<Vec<_>>(),
        "lifecycle":cut.lifecycles.iter().map(|l|json!([l.memory_id.0,l.lifecycle_state,l.archived,l.superseded_by.map(|id|id.0)])).collect::<Vec<_>>(),"updated":cut.updated_at_ns});
    Ok(Sha256::digest(serde_json::to_vec(&value).map_err(|e| e.to_string())?).into())
}
#[path = "dream_batch_validation.rs"]
mod validation;

#[path = "dream_batch_direct.rs"]
pub(crate) mod direct;
