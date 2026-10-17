use super::{RelationshipStore, resolve};
use crate::relationship_model::RelationshipRecord;
use crate::{EntityRef, Relationship, RelationshipError, RelationshipId, RelationshipStats};

impl RelationshipStore {
    pub(crate) fn relationship_revision(
        &self,
        id: RelationshipId,
        revision: u64,
    ) -> Result<Relationship, RelationshipError> {
        self.records
            .iter()
            .find(|record| record.id == id && record.revision == revision)
            .map(resolve)
            .ok_or(RelationshipError::MissingRelationship)
    }

    pub(crate) fn lifecycle_state(
        &self,
        id: RelationshipId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, RelationshipError> {
        if let Some(&coordinates) = self.tombstones.get(&id) {
            return Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(
                coordinates,
            ));
        }
        self.current
            .contains_key(&id)
            .then_some(crate::owner_lifecycle::LifecycleState::Active)
            .ok_or(RelationshipError::MissingRelationship)
    }

    pub(crate) fn retention_descriptor(
        &self,
        id: RelationshipId,
    ) -> Result<crate::relationship_model::RelationshipRetentionDescriptor, RelationshipError> {
        let coordinates = self.tombstones.get(&id).copied().ok_or_else(|| {
            if self.revisions.contains_key(&id) {
                RelationshipError::PayloadRequiredByLiveTarget
            } else {
                RelationshipError::MissingRelationship
            }
        })?;
        let revisions = self
            .records
            .iter()
            .filter(|record| record.id == id)
            .map(
                |record| crate::relationship_model::RelationshipRetainedRevision {
                    revision: record.revision,
                    global_version: record.global_version,
                    relationship_version: record.relationship_version,
                    kind: record.kind.clone(),
                    participants: record.participants.clone(),
                    evidence: record.evidence.clone(),
                    created_at_ns: record.created_at_ns,
                    updated_at_ns: record.updated_at_ns,
                    mutation_id: record.mutation_id.clone(),
                },
            )
            .collect();
        Ok(crate::relationship_model::RelationshipRetentionDescriptor {
            id,
            tombstone_revision: self.revisions[&id],
            tombstone_global_version: coordinates.global_version.unwrap(),
            tombstone_relationship_version: coordinates.domain_version.unwrap(),
            revisions,
        })
    }
    pub(crate) fn relationship(
        &self,
        id: RelationshipId,
    ) -> Result<Relationship, RelationshipError> {
        self.current
            .get(&id)
            .map(|index| resolve(&self.records[*index]))
            .ok_or(RelationshipError::MissingRelationship)
    }

    pub(crate) fn current(&self) -> Vec<Relationship> {
        let mut values: Vec<_> = self
            .current
            .values()
            .map(|index| resolve(&self.records[*index]))
            .collect();
        values.sort_by_key(|value| value.id);
        values
    }

    pub(crate) fn for_entity(&self, entity: &EntityRef) -> Vec<Relationship> {
        self.by_participant
            .get(entity)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.current.get(id))
            .map(|index| resolve(&self.records[*index]))
            .collect()
    }

    pub(crate) fn records(&self) -> &[RelationshipRecord] {
        &self.records
    }

    pub(crate) fn tombstone_global_versions(&self) -> impl Iterator<Item = u64> + '_ {
        self.tombstones
            .values()
            .filter_map(|coordinates| coordinates.global_version)
    }

    pub(crate) fn relationship_version(&self) -> u64 {
        self.next_relationship_version.saturating_sub(1)
    }

    pub(crate) fn stats(&self) -> RelationshipStats {
        RelationshipStats {
            relationships: self.current.len(),
            revisions: self.records.len(),
            relationship_version: self.relationship_version(),
        }
    }
}
