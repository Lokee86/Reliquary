pub use reliquary_perception_store::{
    DEFAULT_ENTITY_RESOLUTION_DORMANT_TTL_NS, DEFAULT_ENTITY_RESOLUTION_PENDING_TTL_NS,
    MAX_ENTITY_RESOLUTION_CANDIDATES,
};

pub use reliquary_memory_core::MemoryEntityMentionKey;

pub use reliquary_perception_store::{
    EntityResolutionDormant, EntityResolutionPending, EntityResolutionReason,
    MemoryEntityResolutionStatus,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntityResolution {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub updated_at_ns: i64,
    pub status: MemoryEntityResolutionStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EntityResolutionCompaction {
    pub dormant: usize,
    pub purged: usize,
    pub rejected_purged: usize,
}
