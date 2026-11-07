//! Named application composition for source-bound semantic lifecycle writes.

use reliquary_owner_session::model::{
    EntityTombstone, MemoryEntityResolution, Relationship, RelationshipTombstone, TerminalTombstone,
};
use reliquary_owner_session::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest, OwnerSessionError,
    PhyWriter, RelWriter, RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};

/// An owner writer selected by the authorized Gateway boundary.
pub enum PerceptionLifecycleWriter<'a> {
    Rel(&'a mut RelWriter),
    Phy(&'a mut PhyWriter),
}

impl PerceptionLifecycleWriter<'_> {
    pub fn withdraw_entity_mention(
        &mut self,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.withdraw_entity_mention(request),
            Self::Phy(writer) => writer.withdraw_entity_mention(request),
        }
    }

    pub fn retire_memory(
        &mut self,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.retire_memory(request),
            Self::Phy(writer) => writer.retire_memory(request),
        }
    }

    pub fn retire_entity(
        &mut self,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.retire_entity(request),
            Self::Phy(writer) => writer.retire_entity(request),
        }
    }

    pub fn create_relationship(
        &mut self,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.create_relationship(request),
            Self::Phy(writer) => writer.create_relationship(request),
        }
    }

    pub fn retire_relationship(
        &mut self,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.retire_relationship(request),
            Self::Phy(writer) => writer.retire_relationship(request),
        }
    }
}
