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
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                let owner_id = self
                    .owner_id()
                    .ok_or(RelationshipError::MissingOwnerIdentity)?;
                self.relationships.publish(
                    &mut self.container,
                    &owner_id,
                    &self.memories,
                    &self.entities,
                    id,
                    expected_revision,
                    draft,
                )
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
