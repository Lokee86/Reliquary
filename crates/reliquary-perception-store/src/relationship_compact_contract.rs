//! Relationship-owned retained revision contract; wire and publication stay separate.
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{RelationshipMemoryRef, RelationshipError, RelationshipId};
use reliquary_container::codec::FieldChange;
use std::collections::BTreeSet;

/// Opaque prune evidence. It proves neither selection nor authorization by itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipDescriptionProof {
    /// SHA-256 of original unpruned record carrier, excluding its publication marker.
    pub original_carrier_digest: [u8; 32],
    /// SHA-256 of the complete canonical encoded CVAHPR01 manifest.
    pub manifest_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipCreation {
    pub record: RelationshipRecord,
    pub description_proof: Option<RelationshipDescriptionProof>,
}

impl RelationshipCreation {
    pub(crate) fn new(record: RelationshipRecord) -> Result<Self, RelationshipError> {
        if record.revision != 1 || record.global_version == 0 || record.relationship_version == 0 {
            return Err(RelationshipError::InvalidVersion);
        }
        crate::relationship_store::validate_record(&record)?;
        Ok(Self {
            record,
            description_proof: None,
        })
    }

    pub(crate) fn new_pruned(
        record: RelationshipRecord,
        proof: RelationshipDescriptionProof,
    ) -> Result<Self, RelationshipError> {
        validate_pruned_summary(&record)?;
        let mut creation = Self::new(record)?;
        creation.description_proof = Some(proof);
        Ok(creation)
    }
}

/// A canonical unordered-set change represented with sorted exact values.
/// Participant role changes are remove-old-tuple plus add-new-tuple.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipCollectionChange<T> {
    pub removed: Vec<T>,
    pub added: Vec<T>,
}

impl<T: Clone + Ord> RelationshipCollectionChange<T> {
    pub(crate) fn between(previous: &[T], next: &[T]) -> Self {
        let previous = previous.iter().cloned().collect::<BTreeSet<_>>();
        let next = next.iter().cloned().collect::<BTreeSet<_>>();
        Self {
            removed: previous.difference(&next).cloned().collect(),
            added: next.difference(&previous).cloned().collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }

    pub(crate) fn validate_canonical(&self) -> Result<(), RelationshipError> {
        if !strictly_sorted(&self.removed)
            || !strictly_sorted(&self.added)
            || self
                .removed
                .iter()
                .any(|value| self.added.binary_search(value).is_ok())
        {
            return Err(RelationshipError::CorruptRecord(
                "noncanonical Relationship collection change",
            ));
        }
        Ok(())
    }
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipDeltaFields {
    pub kind: FieldChange<String>,
    pub participants: RelationshipCollectionChange<RelationshipParticipant>,
    pub evidence: RelationshipCollectionChange<RelationshipMemoryRef>,
    pub summary: FieldChange<String>,
    pub updated_at_ns: FieldChange<i64>,
}

pub(crate) const RELATIONSHIP_DELTA_FIELD_ORDER: &[&str] = &[
    "kind",
    "participants",
    "evidence",
    "summary",
    "updated_at_ns",
];
pub(crate) const RELATIONSHIP_DELTA_KNOWN_MASK: u64 = 0b1_1111;

impl RelationshipDeltaFields {
    pub(crate) fn between(previous: &RelationshipRecord, next: &RelationshipRecord) -> Self {
        let RelationshipRecord {
            id: _,
            revision: _,
            mutation_id: _,
            created_at_ns: _,
            global_version: _,
            relationship_version: _,
            kind: _,
            participants: _,
            evidence: _,
            summary: _,
            updated_at_ns: _,
        } = next;
        Self {
            kind: FieldChange::between(&previous.kind, &next.kind),
            participants: RelationshipCollectionChange::between(
                &previous.participants,
                &next.participants,
            ),
            evidence: RelationshipCollectionChange::between(&previous.evidence, &next.evidence),
            summary: FieldChange::between(&previous.summary, &next.summary),
            updated_at_ns: FieldChange::between(&previous.updated_at_ns, &next.updated_at_ns),
        }
    }

    pub(crate) fn mask(&self) -> u64 {
        u64::from(!matches!(self.kind, FieldChange::Unchanged))
            | (u64::from(!self.participants.is_empty()) << 1)
            | (u64::from(!self.evidence.is_empty()) << 2)
            | (u64::from(!matches!(self.summary, FieldChange::Unchanged)) << 3)
            | (u64::from(!matches!(self.updated_at_ns, FieldChange::Unchanged)) << 4)
    }

    pub(crate) fn validate_canonical(&self) -> Result<(), RelationshipError> {
        if self.participants.removed.len() > crate::MAX_RELATIONSHIP_PARTICIPANTS
            || self.participants.added.len() > crate::MAX_RELATIONSHIP_PARTICIPANTS
            || self.evidence.removed.len() > crate::MAX_RELATIONSHIP_EVIDENCE
            || self.evidence.added.len() > crate::MAX_RELATIONSHIP_EVIDENCE
        {
            return Err(RelationshipError::FieldTooLarge);
        }
        self.participants.validate_canonical()?;
        self.evidence.validate_canonical()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationshipDelta {
    pub id: RelationshipId,
    pub previous_revision: u64,
    pub revision: u64,
    pub global_version: u64,
    pub relationship_version: u64,
    pub mutation_id: String,
    pub fields: RelationshipDeltaFields,
    pub description_proof: Option<RelationshipDescriptionProof>,
}

impl RelationshipDelta {
    pub(crate) fn between(
        previous: &RelationshipRecord,
        next: &RelationshipRecord,
    ) -> Result<Self, RelationshipError> {
        crate::relationship_store::validate_record(previous)?;
        crate::relationship_store::validate_record(next)?;
        if previous.id != next.id {
            return Err(RelationshipError::InvalidField(
                "Relationship delta identity",
            ));
        }
        if previous.revision == 0
            || previous.global_version == 0
            || previous.relationship_version == 0
            || next.global_version == 0
            || next.relationship_version <= previous.relationship_version
        {
            return Err(RelationshipError::InvalidVersion);
        }
        let revision = previous
            .revision
            .checked_add(1)
            .ok_or(RelationshipError::VersionExhausted)?;
        if next.revision != revision || next.created_at_ns != previous.created_at_ns {
            return Err(RelationshipError::RevisionConflict);
        }
        if next.mutation_id == previous.mutation_id {
            return Err(RelationshipError::MutationConflict);
        }
        Ok(Self {
            id: next.id,
            previous_revision: previous.revision,
            revision,
            global_version: next.global_version,
            relationship_version: next.relationship_version,
            mutation_id: next.mutation_id.clone(),
            fields: RelationshipDeltaFields::between(previous, next),
            description_proof: None,
        })
    }

    pub(crate) fn between_pruned(
        previous: &RelationshipRecord,
        next: &RelationshipRecord,
        proof: RelationshipDescriptionProof,
    ) -> Result<Self, RelationshipError> {
        validate_pruned_summary(next)?;
        let mut delta = Self::between(previous, next)?;
        delta.description_proof = Some(proof);
        Ok(delta)
    }

    pub(crate) fn validate_mask(mask: u64) -> Result<(), RelationshipError> {
        if mask & !RELATIONSHIP_DELTA_KNOWN_MASK != 0 {
            return Err(RelationshipError::CorruptRecord(
                "unknown Relationship delta field",
            ));
        }
        Ok(())
    }
}

fn validate_pruned_summary(record: &RelationshipRecord) -> Result<(), RelationshipError> {
    if !record.summary.is_empty() {
        return Err(RelationshipError::CorruptRecord(
            "noncanonical pruned Relationship summary",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "relationship_compact_contract_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "relationship_compact_contract_adversarial_tests.rs"]
mod adversarial_tests;
