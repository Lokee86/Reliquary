//! Relationship state replay and mutation after durable append.
use super::*;
use relationship_codec::RelationshipRecord;
use relationship_model::{relationship_id, validate_relationship};
impl PerceptionStore {
    pub(super) fn write_relationship(
        &mut self,
        c: &mut Container,
        record: RelationshipRecord,
    ) -> Result<(), PerceptionError> {
        let bytes = record.encode()?;
        c.append(&bytes)?;
        c.sync()?;
        self.apply_relationship_record(record)
    }
    pub(super) fn apply_relationship_record(
        &mut self,
        record: RelationshipRecord,
    ) -> Result<(), PerceptionError> {
        if record.owner() != self.owner_id {
            return Err(PerceptionError::OwnerMismatch);
        }
        match record {
            RelationshipRecord::Revision {
                perception_version,
                value,
                ..
            } => {
                if perception_version != self.next_version()?
                    || value.relationship_version
                        != self
                            .relationship_version
                            .checked_add(1)
                            .ok_or(PerceptionError::VersionExhausted)?
                    || self.relationship_tombstones.contains_key(&value.id)
                {
                    return Err(PerceptionError::CorruptRecord(
                        "Relationship version or terminal state",
                    ));
                }
                validate_relationship(&value.as_draft())?;
                let previous = self.relationships.get(&value.id);
                let expected = previous.map_or(1, |r| r.revision.checked_add(1).unwrap_or(0));
                if value.revision != expected
                    || self.relationship_mutations.contains_key(&value.mutation_id)
                    || previous.is_none() && value.id != relationship_id(&value.mutation_id)
                    || previous.is_some_and(|r| {
                        r.created_at_ns != value.created_at_ns
                            || r.updated_at_ns > value.updated_at_ns
                    })
                {
                    return Err(PerceptionError::CorruptRecord(
                        "Relationship revision identity",
                    ));
                }
                self.relationship_mutations
                    .insert(value.mutation_id.clone(), (value.id, value.revision));
                self.relationship_history
                    .insert((value.id, value.revision), value.clone());
                self.relationship_versions
                    .insert((value.id, value.revision), perception_version);
                self.relationships.insert(value.id, value.clone());
                self.relationship_version = value.relationship_version;
                self.version = perception_version;
            }
            RelationshipRecord::Tombstone {
                perception_version,
                value,
                ..
            } => {
                if perception_version != self.next_version()?
                    || value.relationship_version
                        != self
                            .relationship_version
                            .checked_add(1)
                            .ok_or(PerceptionError::VersionExhausted)?
                    || self.relationship_tombstones.contains_key(&value.id)
                    || self
                        .relationships
                        .get(&value.id)
                        .is_none_or(|r| r.revision != value.revision)
                {
                    return Err(PerceptionError::CorruptRecord(
                        "Relationship tombstone evidence",
                    ));
                }
                self.relationships.remove(&value.id);
                self.relationship_tombstone_versions
                    .insert(value.id, perception_version);
                self.relationship_tombstones.insert(value.id, value);
                self.relationship_version = value.relationship_version;
                self.version = perception_version;
            }
            RelationshipRecord::Prune { value, .. } => {
                if value.manifest_digest == [0; 32]
                    || value.summary_digest == [0; 32]
                    || self
                        .relationship_history
                        .get(&(value.id, value.revision))
                        .is_none_or(|r| !r.summary.is_empty())
                    || self
                        .pruned_relationship_descriptions
                        .contains_key(&(value.id, value.revision))
                {
                    return Err(PerceptionError::CorruptRecord("Relationship prune proof"));
                }
                self.pruned_relationship_descriptions
                    .insert((value.id, value.revision), value);
            }
        }
        Ok(())
    }
}
