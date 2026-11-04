//! Owner-validated cut for one durable, zero-relation Dream evaluation.
//! These immutable values are not store handles or classifier responses.
use reliquary_memory_core::{MemoryBodyId, MemoryId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamMemoryCut {
    pub id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamNoLinkCut {
    pub owner_id: [u8; 16],
    pub graph_version: u64,
    pub source: DreamMemoryCut,
    pub candidate: DreamMemoryCut,
}
