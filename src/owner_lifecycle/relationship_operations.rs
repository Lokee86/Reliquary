use crate::{
    EntityRef, Relationship, RelationshipDraft, RelationshipError, RelationshipId,
    RelationshipStats,
};
macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn native_publish_relationship(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                mut draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                let owner_id = self
                    .owner_id()
                    .ok_or(RelationshipError::MissingOwnerIdentity)?;
                crate::owner_lifecycle::validate_live_relationship_refs(
                    &owner_id,
                    &self.memories,
                    &self.entities,
                    &mut draft,
                )?;
                self.relationships
                    .publish(&mut self.container, id, expected_revision, draft)
            }
            pub(crate) fn native_replay_relationship_revision(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                self.relationships.publish_replayed(
                    &mut self.container,
                    id,
                    expected_revision,
                    draft,
                )
            }
            pub(crate) fn native_relationship(
                &self,
                id: RelationshipId,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship(id)
            }
            pub(crate) fn native_tombstone_relationship(
                &mut self,
                id: RelationshipId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
                self.relationships
                    .tombstone(&mut self.container, id, expected_revision)
            }
            pub(crate) fn native_relationship_retention_descriptor(
                &self,
                id: RelationshipId,
            ) -> Result<crate::RelationshipRetentionDescriptor, RelationshipError> {
                self.relationships.retention_descriptor(id)
            }
            pub(crate) fn native_historical_relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<
                crate::owner_lifecycle::HistoricalPayload<
                    Relationship,
                    crate::RelationshipRetainedRevision,
                >,
                RelationshipError,
            > {
                let value = match self.native_relationship_revision(id, revision) {
                    Ok(value) => value,
                    Err(RelationshipError::PayloadUnavailable(reason)) => {
                        let structure = self
                            .native_relationship_retention_descriptor(id)?
                            .revisions
                            .into_iter()
                            .find(|r| r.revision == revision)
                            .ok_or(RelationshipError::MissingRelationship)?;
                        return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                            structural_history: structure,
                            reason,
                        });
                    }
                    Err(error) => return Err(error),
                };
                Ok(match self.native_relationship_lifecycle_state(id)? {
                    crate::owner_lifecycle::LifecycleState::Active => {
                        crate::owner_lifecycle::HistoricalPayload::Available(value)
                    }
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                        crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
                    }
                })
            }
            pub(crate) fn native_relationship_lifecycle_state(
                &self,
                id: RelationshipId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, RelationshipError> {
                self.relationships.lifecycle_state(id)
            }
            pub(crate) fn native_relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship_revision(id, revision)
            }
            pub(crate) fn native_relationships(&self) -> Vec<Relationship> {
                self.relationships.current()
            }
            pub(crate) fn native_relationships_for_entity(
                &self,
                entity: &EntityRef,
            ) -> Vec<Relationship> {
                self.relationships.for_entity(entity)
            }
            pub(crate) fn native_relationship_version(&self) -> u64 {
                self.relationships.relationship_version()
            }
            pub(crate) fn native_relationship_stats(&self) -> RelationshipStats {
                self.relationships.stats()
            }
        }
    };
}
impl_owner!(crate::owner_lifecycle::ReliquaryOwner);
impl_owner!(crate::owner_lifecycle::PhylacteryOwner);

/// Normalize and validate relationships against concrete REL/PHY owner stores.
pub(crate) fn validate_live_relationship_refs(
    owner_id: &str,
    memories: &crate::memory_store::MemoryStore,
    entities: &crate::entity_store::EntityStore,
    draft: &mut RelationshipDraft,
) -> Result<(), RelationshipError> {
    crate::relationship_store::normalize_draft(draft)?;
    crate::owner_lifecycle::validate_relationship_entities(
        owner_id,
        entities,
        &draft.participants,
        false,
    )?;
    for evidence in &draft.evidence {
        if evidence.owner_id == owner_id && !memories.contains_memory(evidence.memory_id) {
            return Err(RelationshipError::MissingLocalMemory(evidence.memory_id));
        }
    }
    Ok(())
}
