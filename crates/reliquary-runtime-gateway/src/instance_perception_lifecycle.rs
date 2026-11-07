use super::*;
use reliquary_application::PerceptionLifecycleWriter;
use reliquary_owner_session::model::{
    EntityTombstone, MemoryEntityResolution, Relationship, RelationshipTombstone, TerminalTombstone,
};
use reliquary_owner_session::{
    CreateRelationshipRequest, EntityRetirementRequest, MemoryRetirementRequest,
    RelationshipRetirementRequest, WithdrawEntityMentionRequest,
};

impl Instance {
    /// Withdraw a resolved mention by exact source cut and current mention revision.
    pub fn withdraw_semantic_entity_mention(
        &self,
        owner: OwnerId,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, GatewayError> {
        ensure_owner(owner, request.owner_id)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                PerceptionLifecycleWriter::Rel(writer).withdraw_entity_mention(request)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                PerceptionLifecycleWriter::Phy(writer).withdraw_entity_mention(request)
            })??)
        }
    }

    /// Retire an archived Memory through an owner grant and source-bound application workflow.
    pub fn retire_semantic_memory(
        &self,
        owner: OwnerId,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, GatewayError> {
        ensure_owner(owner, request.owner_id)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                PerceptionLifecycleWriter::Rel(writer).retire_memory(request)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                PerceptionLifecycleWriter::Phy(writer).retire_memory(request)
            })??)
        }
    }

    /// Retire an unreferenced Entity with exact evidence from a current Memory span.
    pub fn retire_semantic_entity(
        &self,
        owner: OwnerId,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, GatewayError> {
        ensure_owner(owner, request.owner_id)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                PerceptionLifecycleWriter::Rel(writer).retire_entity(request)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                PerceptionLifecycleWriter::Phy(writer).retire_entity(request)
            })??)
        }
    }

    /// Create a Relationship from exact accepted Memory evidence and current local Entities.
    pub fn publish_semantic_relationship(
        &self,
        owner: OwnerId,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, GatewayError> {
        ensure_owner(owner, request.owner_id)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                PerceptionLifecycleWriter::Rel(writer).create_relationship(request)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                PerceptionLifecycleWriter::Phy(writer).create_relationship(request)
            })??)
        }
    }

    /// Retire a Relationship with an exact current source Memory witness.
    pub fn retire_semantic_relationship(
        &self,
        owner: OwnerId,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, GatewayError> {
        ensure_owner(owner, request.owner_id)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                PerceptionLifecycleWriter::Rel(writer).retire_relationship(request)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                PerceptionLifecycleWriter::Phy(writer).retire_relationship(request)
            })??)
        }
    }
}

fn ensure_owner(owner: OwnerId, request_owner: [u8; 16]) -> Result<(), GatewayError> {
    if owner.as_bytes() == request_owner {
        Ok(())
    } else {
        Err(OwnerSessionError::Record("semantic lifecycle owner mismatch".into()).into())
    }
}
