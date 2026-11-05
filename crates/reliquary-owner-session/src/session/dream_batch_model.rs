//! Immutable, bounded Application decisions admitted by one owner writer.
use super::*;
use crate::DreamMemoryCut;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamPairPublication {
    pub source: MemoryId,
    pub candidate: MemoryId,
    pub kind: Option<reliquary_graph::RelationKind>,
    pub rejected: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamLifecyclePublication {
    pub memory_id: MemoryId,
    pub lifecycle_state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamBatchCut {
    pub owner: [u8; 16],
    pub publication_id: [u8; 32],
    pub graph_version: u64,
    pub observed: Vec<DreamMemoryCut>,
    pub pairs: Vec<DreamPairPublication>,
    pub changes: Vec<reliquary_graph::RelationChange>,
    pub lifecycles: Vec<DreamLifecyclePublication>,
    pub updated_at_ns: i64,
}
