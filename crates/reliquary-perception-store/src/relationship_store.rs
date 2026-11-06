//! Relationship lifecycle and replay, owned by Perception.
use super::*;
use relationship_codec::RelationshipRecord;
use relationship_model::{relationship_id, validate_relationship};
impl PerceptionStore {
    pub fn relationship(&self, id: RelationshipId) -> Option<&Relationship> {
        self.relationships.get(&id)
    }
    pub fn relationship_revision(
        &self,
        id: RelationshipId,
        revision: u64,
    ) -> Option<&Relationship> {
        self.relationship_history.get(&(id, revision))
    }
    pub fn relationships(&self) -> impl Iterator<Item = &Relationship> {
        self.relationships.values()
    }
    pub fn historical_relationships(&self) -> impl Iterator<Item = RelationshipId> + '_ {
        self.relationship_tombstones.keys().copied()
    }
    pub fn relationship_tombstone(&self, id: RelationshipId) -> Option<RelationshipTombstone> {
        self.relationship_tombstones.get(&id).copied()
    }
    pub fn relationship_description_prunes(
        &self,
    ) -> impl Iterator<Item = RelationshipDescriptionPrune> + '_ {
        self.pruned_relationship_descriptions.values().copied()
    }
    pub fn relationship_description_prune(
        &self,
        id: RelationshipId,
        revision: u64,
    ) -> Option<RelationshipDescriptionPrune> {
        self.pruned_relationship_descriptions
            .get(&(id, revision))
            .copied()
    }
    pub fn relationship_version(&self) -> u64 {
        self.relationship_version
    }
    pub fn publish_relationship(
        &mut self,
        c: &mut Container,
        draft: RelationshipDraft,
    ) -> Result<Relationship, PerceptionError> {
        self.check_owner(c)?;
        validate_relationship(&draft)?;
        if let Some(&(id, revision)) = self.relationship_mutations.get(&draft.mutation_id) {
            let old = self.relationship_history.get(&(id, revision)).ok_or(
                PerceptionError::CorruptRecord("missing Relationship mutation"),
            )?;
            return if old.as_draft() == draft {
                Ok(old.clone())
            } else {
                Err(PerceptionError::InvalidField(
                    "conflicting Relationship mutation",
                ))
            };
        }
        let id = relationship_id(&draft.mutation_id);
        if self.relationships.contains_key(&id) || self.relationship_tombstones.contains_key(&id) {
            return Err(PerceptionError::InvalidField(
                "duplicate Relationship identity",
            ));
        }
        let value = Relationship {
            id,
            revision: 1,
            kind: draft.kind,
            participants: draft.participants,
            evidence: draft.evidence,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            relationship_version: self
                .relationship_version
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?,
        };
        self.write_relationship(
            c,
            RelationshipRecord::Revision {
                owner: self.owner_id,
                perception_version: self.next_version()?,
                value: value.clone(),
            },
        )?;
        Ok(value)
    }
    pub fn update_relationship(
        &mut self,
        c: &mut Container,
        id: RelationshipId,
        expected_revision: u64,
        draft: RelationshipDraft,
    ) -> Result<Relationship, PerceptionError> {
        self.check_owner(c)?;
        validate_relationship(&draft)?;
        if let Some(&(mut_id, revision)) = self.relationship_mutations.get(&draft.mutation_id) {
            let old = self.relationship_history.get(&(mut_id, revision)).ok_or(
                PerceptionError::CorruptRecord("missing Relationship mutation"),
            )?;
            if mut_id == id
                && expected_revision.checked_add(1) == Some(revision)
                && old.as_draft() == draft
            {
                return Ok(old.clone());
            }
            return Err(PerceptionError::InvalidField(
                "conflicting Relationship mutation",
            ));
        }
        let previous = self
            .relationships
            .get(&id)
            .ok_or(PerceptionError::InvalidField("missing Relationship"))?;
        if previous.revision != expected_revision
            || previous.created_at_ns != draft.created_at_ns
            || previous.updated_at_ns > draft.updated_at_ns
        {
            return Err(PerceptionError::InvalidField("Relationship revision/time"));
        }
        let value = Relationship {
            id,
            revision: previous
                .revision
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?,
            kind: draft.kind,
            participants: draft.participants,
            evidence: draft.evidence,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: draft.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
            relationship_version: self
                .relationship_version
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?,
        };
        self.write_relationship(
            c,
            RelationshipRecord::Revision {
                owner: self.owner_id,
                perception_version: self.next_version()?,
                value: value.clone(),
            },
        )?;
        Ok(value)
    }
    pub fn tombstone_relationship(
        &mut self,
        c: &mut Container,
        id: RelationshipId,
        revision: u64,
    ) -> Result<RelationshipTombstone, PerceptionError> {
        self.check_owner(c)?;
        if let Some(old) = self.relationship_tombstones.get(&id) {
            return if old.revision == revision {
                Ok(*old)
            } else {
                Err(PerceptionError::InvalidField(
                    "Relationship tombstone revision",
                ))
            };
        }
        if self
            .relationships
            .get(&id)
            .is_none_or(|r| r.revision != revision)
        {
            return Err(PerceptionError::InvalidField(
                "Relationship tombstone target",
            ));
        }
        let value = RelationshipTombstone {
            id,
            revision,
            relationship_version: self
                .relationship_version
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?,
        };
        self.write_relationship(
            c,
            RelationshipRecord::Tombstone {
                owner: self.owner_id,
                perception_version: self.next_version()?,
                value,
            },
        )?;
        Ok(value)
    }
}
