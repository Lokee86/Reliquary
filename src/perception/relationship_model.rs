use crate::{EntityRef, MemoryRef};

pub const MAX_RELATIONSHIP_KIND_BYTES: usize = 128;
pub const MAX_RELATIONSHIP_ROLE_BYTES: usize = 128;
pub const MAX_RELATIONSHIP_SUMMARY_BYTES: usize = 4096;
pub const MAX_RELATIONSHIP_OWNER_ID_BYTES: usize = 128;
pub const MAX_RELATIONSHIP_PARTICIPANTS: usize = 64;
pub const MAX_RELATIONSHIP_EVIDENCE: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipParticipant {
    pub entity: EntityRef,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipDraft {
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<MemoryRef>,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relationship {
    pub id: RelationshipId,
    pub revision: u64,
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<MemoryRef>,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
    pub global_version: u64,
    pub relationship_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationshipStats {
    pub relationships: usize,
    pub revisions: usize,
    pub relationship_version: u64,
}

/// Minimum structural evidence retained for a tombstoned Relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRetentionDescriptor {
    pub id: RelationshipId,
    pub tombstone_revision: u64,
    pub tombstone_global_version: u64,
    pub tombstone_relationship_version: u64,
    pub revisions: Vec<RelationshipRetainedRevision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRetainedRevision {
    pub revision: u64,
    pub global_version: u64,
    pub relationship_version: u64,
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<MemoryRef>,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
    pub mutation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipRecord {
    pub id: RelationshipId,
    pub revision: u64,
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<MemoryRef>,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
    pub global_version: u64,
    pub relationship_version: u64,
}
