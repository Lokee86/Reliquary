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
                self.owner
                    .native_publish_relationship(id, expected_revision, draft)
            }
            pub(crate) fn native_replay_relationship_revision(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                self.owner
                    .native_replay_relationship_revision(id, expected_revision, draft)
            }
            pub(crate) fn native_relationship(
                &self,
                id: RelationshipId,
            ) -> Result<Relationship, RelationshipError> {
                self.owner.native_relationship(id)
            }
            pub(crate) fn native_tombstone_relationship(
                &mut self,
                id: RelationshipId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
                self.owner
                    .native_tombstone_relationship(id, expected_revision)
            }
            pub(crate) fn native_relationship_retention_descriptor(
                &self,
                id: RelationshipId,
            ) -> Result<crate::RelationshipRetentionDescriptor, RelationshipError> {
                self.owner.native_relationship_retention_descriptor(id)
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
                self.owner
                    .native_historical_relationship_revision(id, revision)
            }
            pub(crate) fn native_relationship_lifecycle_state(
                &self,
                id: RelationshipId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, RelationshipError> {
                self.owner.native_relationship_lifecycle_state(id)
            }
            pub(crate) fn native_relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<Relationship, RelationshipError> {
                self.owner.native_relationship_revision(id, revision)
            }
            pub(crate) fn native_relationships(&self) -> Vec<Relationship> {
                self.owner.native_relationships()
            }
            pub(crate) fn native_relationships_for_entity(
                &self,
                entity: &EntityRef,
            ) -> Vec<Relationship> {
                self.owner.native_relationships_for_entity(entity)
            }
            pub(crate) fn native_relationship_version(&self) -> u64 {
                self.owner.native_relationship_version()
            }
            pub(crate) fn native_relationship_stats(&self) -> RelationshipStats {
                self.owner.native_relationship_stats()
            }
        }
    };
}
impl_owner!(crate::RelExecutionOwner);
impl_owner!(crate::phy_execution_owner::PhyExecutionOwner);
