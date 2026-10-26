use crate::{EntityRef, MemoryRef};
use reliquary_perception_store::{RelationshipMemoryRef, RelationshipParticipant as StoreParticipant};

pub use reliquary_perception_store::{
    RelationshipId, RelationshipStats, MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES,
    MAX_RELATIONSHIP_OWNER_ID_BYTES, MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES,
    MAX_RELATIONSHIP_SUMMARY_BYTES,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelationshipParticipant {
    pub entity: EntityRef,
    pub role: Option<String>,
}

impl From<RelationshipParticipant> for StoreParticipant {
    fn from(value: RelationshipParticipant) -> Self {
        Self { entity: value.entity, role: value.role }
    }
}
impl From<StoreParticipant> for RelationshipParticipant {
    fn from(value: StoreParticipant) -> Self {
        Self { entity: value.entity, role: value.role }
    }
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
impl From<RelationshipDraft> for reliquary_perception_store::RelationshipDraft {
    fn from(v: RelationshipDraft) -> Self {
        Self { kind:v.kind, participants:v.participants.into_iter().map(Into::into).collect(),
            evidence:v.evidence.into_iter().map(|r| RelationshipMemoryRef { owner_id:r.owner_id, memory_id:r.memory_id.0 }).collect(),
            summary:v.summary, mutation_id:v.mutation_id, created_at_ns:v.created_at_ns, updated_at_ns:v.updated_at_ns }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relationship {
    pub id: RelationshipId, pub revision: u64, pub kind: String,
    pub participants: Vec<RelationshipParticipant>, pub evidence: Vec<MemoryRef>,
    pub summary: String, pub mutation_id: String, pub created_at_ns: i64,
    pub updated_at_ns: i64, pub global_version: u64, pub relationship_version: u64,
}
impl From<reliquary_perception_store::Relationship> for Relationship {
    fn from(v: reliquary_perception_store::Relationship) -> Self {
        Self { id:v.id, revision:v.revision, kind:v.kind, participants:v.participants.into_iter().map(Into::into).collect(),
            evidence:v.evidence.into_iter().map(|r| MemoryRef { owner_id:r.owner_id, memory_id: crate::MemoryId(r.memory_id) }).collect(),
            summary:v.summary, mutation_id:v.mutation_id, created_at_ns:v.created_at_ns,
            updated_at_ns:v.updated_at_ns, global_version:v.global_version, relationship_version:v.relationship_version }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRetainedRevision {
    pub revision:u64, pub global_version:u64, pub relationship_version:u64, pub kind:String,
    pub participants:Vec<RelationshipParticipant>, pub evidence:Vec<MemoryRef>, pub created_at_ns:i64,
    pub updated_at_ns:i64, pub mutation_id:String,
}
impl From<reliquary_perception_store::RelationshipRetainedRevision> for RelationshipRetainedRevision {
    fn from(v: reliquary_perception_store::RelationshipRetainedRevision) -> Self {
        Self { revision:v.revision, global_version:v.global_version, relationship_version:v.relationship_version,
            kind:v.kind, participants:v.participants.into_iter().map(Into::into).collect(),
            evidence:v.evidence.into_iter().map(|r| MemoryRef { owner_id:r.owner_id, memory_id:crate::MemoryId(r.memory_id) }).collect(),
            created_at_ns:v.created_at_ns, updated_at_ns:v.updated_at_ns, mutation_id:v.mutation_id }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRetentionDescriptor {
    pub id:RelationshipId, pub tombstone_revision:u64, pub tombstone_global_version:u64,
    pub tombstone_relationship_version:u64, pub revisions:Vec<RelationshipRetainedRevision>,
}
impl From<reliquary_perception_store::RelationshipRetentionDescriptor> for RelationshipRetentionDescriptor {
    fn from(v: reliquary_perception_store::RelationshipRetentionDescriptor) -> Self {
        Self { id:v.id, tombstone_revision:v.tombstone_revision, tombstone_global_version:v.tombstone_global_version,
            tombstone_relationship_version:v.tombstone_relationship_version,
            revisions:v.revisions.into_iter().map(Into::into).collect() }
    }
}
