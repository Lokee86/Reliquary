use crate::memory_store::MemoryStore;
use crate::{
    Cva, EntityRef, MemoryId, Phylactery, Relationship, RelationshipDraft, RelationshipError,
    RelationshipId, RelationshipStats,
};
use reliquary_perception_store::EntityStore;

fn validate_live_local_references(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    draft: &reliquary_perception_store::RelationshipDraft,
) -> Result<(), RelationshipError> {
    for participant in &draft.participants {
        if participant.entity.owner_id == owner_id
            && !entities.contains(participant.entity.entity_id)
        {
            return Err(RelationshipError::MissingLocalEntity(
                participant.entity.entity_id,
            ));
        }
    }
    for evidence in &draft.evidence {
        if evidence.owner_id == owner_id && !memories.contains_memory(MemoryId(evidence.memory_id))
        {
            return Err(RelationshipError::MissingLocalMemory(MemoryId(
                evidence.memory_id,
            )));
        }
    }
    Ok(())
}

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn publish_relationship(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                let owner_id = self
                    .owner_id()
                    .ok_or(RelationshipError::MissingOwnerIdentity)?;
                let mut draft: reliquary_perception_store::RelationshipDraft = draft.into();
                reliquary_perception_store::relationship_store_validation::normalize_draft(&mut draft)?;
                validate_live_local_references(&owner_id, &self.memories, &self.entities, &draft)?;
                self.relationships.publish(&mut self.container, id, expected_revision, draft)
                    .map(|(value, changed)| (value.into(), changed))
                    .map_err(RelationshipError::from)
            }

            pub(crate) fn replay_relationship_revision(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                self.relationships.publish_replayed(
                    &mut self.container,
                    id,
                    expected_revision,
                    draft.into(),
                ).map(|(value, changed)| (value.into(), changed)).map_err(RelationshipError::from)
            }

            pub fn relationship(
                &self,
                id: RelationshipId,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship(id)
                    .map(Into::into)
                    .map_err(RelationshipError::from)
            }

            pub fn tombstone_relationship(
                &mut self,
                id: RelationshipId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
                Ok(match self.relationships.tombstone(&mut self.container, id, expected_revision)? {
                    reliquary_perception_store::RelationshipTombstoneOutcome::Published(c) =>
                        crate::owner_lifecycle::TombstoneOutcome::Published(crate::owner_lifecycle::TombstoneCoordinates { global_version:c.global_version, domain_version:c.domain_version }),
                    reliquary_perception_store::RelationshipTombstoneOutcome::AlreadyTombstoned(c) =>
                        crate::owner_lifecycle::TombstoneOutcome::AlreadyTombstoned(crate::owner_lifecycle::TombstoneCoordinates { global_version:c.global_version, domain_version:c.domain_version }),
                })
            }

            pub fn relationship_retention_descriptor(
                &self,
                id: RelationshipId,
            ) -> Result<crate::RelationshipRetentionDescriptor, RelationshipError> {
                self.relationships.retention_descriptor(id).map(Into::into).map_err(RelationshipError::from)
            }

            pub fn historical_relationship_revision(
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
                let value = match self.relationship_revision(id, revision) {
                    Ok(value) => value,
                    Err(RelationshipError::PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable::Pruned)) => {
                        let structure = self
                            .relationship_retention_descriptor(id)?
                            .revisions
                            .into_iter()
                            .find(|r| r.revision == revision)
                            .ok_or(RelationshipError::MissingRelationship)?;
                        return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                            structural_history: structure,
                            reason: crate::owner_lifecycle::PayloadUnavailable::Pruned,
                        });
                    }
                    Err(error) => return Err(error),
                };
                Ok(match self.relationship_lifecycle_state(id)? {
                    crate::owner_lifecycle::LifecycleState::Active => {
                        crate::owner_lifecycle::HistoricalPayload::Available(value)
                    }
                    crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                        crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
                    }
                })
            }

            pub fn relationship_lifecycle_state(
                &self,
                id: RelationshipId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, RelationshipError> {
                Ok(match self.relationships.lifecycle_state(id).map_err(RelationshipError::from)? {
                    reliquary_perception_store::RelationshipLifecycleState::Active => crate::owner_lifecycle::LifecycleState::Active,
                    reliquary_perception_store::RelationshipLifecycleState::Tombstoned(c) => crate::owner_lifecycle::LifecycleState::Tombstoned(crate::owner_lifecycle::TombstoneCoordinates { global_version:c.global_version, domain_version:c.domain_version }),
                })
            }

            pub fn relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship_revision(id, revision).map(Into::into).map_err(RelationshipError::from)
            }

            pub fn relationships(&self) -> Vec<Relationship> {
                self.relationships.current().into_iter().map(Into::into).collect()
            }

            pub fn relationships_for_entity(&self, entity: &EntityRef) -> Vec<Relationship> {
                self.relationships.for_entity(entity).into_iter().map(Into::into).collect()
            }

            pub fn relationship_version(&self) -> u64 {
                self.relationships.relationship_version()
            }

            pub fn relationship_stats(&self) -> RelationshipStats {
                self.relationships.stats()
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
