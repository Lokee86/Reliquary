use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{
    Cva, EntityRef, Phylactery, Relationship, RelationshipDraft, RelationshipError, RelationshipId,
    RelationshipStats,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn publish_relationship(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                mut draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                let owner_id = self
                    .owner_id()
                    .ok_or(RelationshipError::MissingOwnerIdentity)?;
                crate::relationship_store::normalize_draft(&mut draft)?;
                validate_live_relationship_refs(&owner_id, &self.memories, &self.entities, &draft)?;
                self.relationships
                    .publish(&mut self.container, id, expected_revision, draft)
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
                    draft,
                )
            }

            pub fn relationship(
                &self,
                id: RelationshipId,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship(id)
            }

            pub fn tombstone_relationship(
                &mut self,
                id: RelationshipId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
                self.relationships
                    .tombstone(&mut self.container, id, expected_revision)
            }

            pub fn relationship_retention_descriptor(
                &self,
                id: RelationshipId,
            ) -> Result<crate::RelationshipRetentionDescriptor, RelationshipError> {
                self.relationships.retention_descriptor(id)
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
                    Err(RelationshipError::PayloadUnavailable(reason)) => {
                        let structure = self
                            .relationship_retention_descriptor(id)?
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
                self.relationships.lifecycle_state(id)
            }

            pub fn relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<Relationship, RelationshipError> {
                self.relationships.relationship_revision(id, revision)
            }

            pub fn relationships(&self) -> Vec<Relationship> {
                self.relationships.current()
            }

            pub fn relationships_for_entity(&self, entity: &EntityRef) -> Vec<Relationship> {
                self.relationships.for_entity(entity)
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

pub(crate) fn validate_current_relationship_references(
    owner_id: Option<&str>,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &[Relationship],
) -> Result<(), RelationshipError> {
    if relationships.is_empty() {
        return Ok(());
    }
    let owner_id = owner_id.ok_or(RelationshipError::MissingOwnerIdentity)?;
    for relationship in relationships {
        validate_historical_relationship_refs(
            owner_id,
            memories,
            entities,
            &relationship.participants,
            &relationship.evidence,
        )?;
    }
    Ok(())
}

fn validate_live_relationship_refs(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    draft: &RelationshipDraft,
) -> Result<(), RelationshipError> {
    validate_relationship_entities(owner_id, entities, &draft.participants, false)?;
    for evidence in &draft.evidence {
        if evidence.owner_id == owner_id && !memories.contains_memory(evidence.memory_id) {
            return Err(RelationshipError::MissingLocalMemory(evidence.memory_id));
        }
    }
    Ok(())
}

fn validate_historical_relationship_refs(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    evidence: &[crate::MemoryRef],
) -> Result<(), RelationshipError> {
    validate_relationship_entities(owner_id, entities, participants, true)?;
    for evidence in evidence {
        if evidence.owner_id == owner_id && !memories.contains_historical_memory(evidence.memory_id)
        {
            return Err(RelationshipError::MissingLocalMemory(evidence.memory_id));
        }
    }
    Ok(())
}

fn validate_relationship_entities(
    owner_id: &str,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    historical: bool,
) -> Result<(), RelationshipError> {
    for participant in participants {
        if participant.entity.owner_id != owner_id {
            continue;
        }
        let exists = if historical {
            entities.contains_historical(participant.entity.entity_id)
        } else {
            entities.contains(participant.entity.entity_id)
        };
        if !exists {
            return Err(RelationshipError::MissingLocalEntity(
                participant.entity.entity_id,
            ));
        }
    }
    Ok(())
}
