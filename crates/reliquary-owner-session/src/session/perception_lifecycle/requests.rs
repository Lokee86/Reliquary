use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRetirementRequest {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub memory_revision: u64,
    pub body_id: MemoryBodyId,
    pub superseded_by: MemoryId,
    pub successor_revision: u64,
    pub successor_body_id: MemoryBodyId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRetirementRequest {
    pub owner_id: [u8; 16],
    pub entity_id: EntityId,
    pub entity_revision: u64,
    pub source_memory_id: MemoryId,
    pub source_memory_revision: u64,
    pub source_body_id: MemoryBodyId,
    pub source_field: MemoryTextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_quote: String,
}

/// User-directed withdrawal bound to the exact active mention and Memory cut.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawEntityMentionRequest {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub memory_revision: u64,
    pub body_id: MemoryBodyId,
    pub key: MemoryEntityMentionKey,
    pub source_surface: String,
    pub expected_mention_revision: u32,
    pub target_revisions: Vec<(EntityId, u64)>,
    pub withdrawn_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipParticipantEvidence {
    pub entity_id: EntityId,
    pub entity_revision: u64,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRelationshipRequest {
    pub owner_id: [u8; 16],
    pub kind: String,
    pub participants: Vec<RelationshipParticipantEvidence>,
    pub source_memory_id: MemoryId,
    pub source_memory_revision: u64,
    pub source_body_id: MemoryBodyId,
    pub source_field: MemoryTextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_quote: String,
    pub mutation_id: String,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipRetirementRequest {
    pub owner_id: [u8; 16],
    pub relationship_id: RelationshipId,
    pub relationship_revision: u64,
    pub source_memory_id: MemoryId,
    pub source_memory_revision: u64,
    pub source_body_id: MemoryBodyId,
    pub source_field: MemoryTextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_quote: String,
}
