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
                self.native_publish_relationship(id, expected_revision, draft)
            }

            pub(crate) fn replay_relationship_revision(
                &mut self,
                id: Option<RelationshipId>,
                expected_revision: u64,
                draft: RelationshipDraft,
            ) -> Result<(Relationship, bool), RelationshipError> {
                self.native_replay_relationship_revision(id, expected_revision, draft)
            }

            pub fn relationship(
                &self,
                id: RelationshipId,
            ) -> Result<Relationship, RelationshipError> {
                self.native_relationship(id)
            }

            pub fn tombstone_relationship(
                &mut self,
                id: RelationshipId,
                expected_revision: u64,
            ) -> Result<crate::owner_lifecycle::TombstoneOutcome, RelationshipError> {
                self.native_tombstone_relationship(id, expected_revision)
            }

            pub fn relationship_retention_descriptor(
                &self,
                id: RelationshipId,
            ) -> Result<crate::RelationshipRetentionDescriptor, RelationshipError> {
                self.native_relationship_retention_descriptor(id)
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
                self.native_historical_relationship_revision(id, revision)
            }

            pub fn relationship_lifecycle_state(
                &self,
                id: RelationshipId,
            ) -> Result<crate::owner_lifecycle::LifecycleState, RelationshipError> {
                self.native_relationship_lifecycle_state(id)
            }

            pub fn relationship_revision(
                &self,
                id: RelationshipId,
                revision: u64,
            ) -> Result<Relationship, RelationshipError> {
                self.native_relationship_revision(id, revision)
            }

            pub fn relationships(&self) -> Vec<Relationship> {
                self.native_relationships()
            }

            pub fn relationships_for_entity(&self, entity: &EntityRef) -> Vec<Relationship> {
                self.native_relationships_for_entity(entity)
            }

            pub fn relationship_version(&self) -> u64 {
                self.native_relationship_version()
            }

            pub fn relationship_stats(&self) -> RelationshipStats {
                self.native_relationship_stats()
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
