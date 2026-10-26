//! Pure Relationship candidate reconstruction; store admission owns references/publication.
use crate::RelationshipError;
use crate::relationship_compact_contract::{RelationshipCollectionChange, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use reliquary_container::codec::apply_field;
use std::collections::BTreeSet;

pub(crate) fn apply_delta(
    previous: &RelationshipRecord,
    delta: &RelationshipDelta,
) -> Result<RelationshipRecord, RelationshipError> {
    if previous.id != delta.id {
        return Err(RelationshipError::InvalidField(
            "Relationship delta identity",
        ));
    }
    if delta.previous_revision != previous.revision
        || previous.revision.checked_add(1) != Some(delta.revision)
    {
        return Err(RelationshipError::RevisionConflict);
    }
    let candidate = RelationshipRecord {
        id: previous.id,
        revision: delta.revision,
        kind: apply_field(&previous.kind, &delta.fields.kind),
        participants: apply_collection(&previous.participants, &delta.fields.participants)?,
        evidence: apply_collection(&previous.evidence, &delta.fields.evidence)?,
        summary: apply_field(&previous.summary, &delta.fields.summary),
        mutation_id: delta.mutation_id.clone(),
        created_at_ns: previous.created_at_ns,
        updated_at_ns: apply_field(&previous.updated_at_ns, &delta.fields.updated_at_ns),
        global_version: delta.global_version,
        relationship_version: delta.relationship_version,
    };
    let canonical = match &delta.description_proof {
        Some(proof) => RelationshipDelta::between_pruned(previous, &candidate, proof.clone())?,
        None => RelationshipDelta::between(previous, &candidate)?,
    };
    if canonical != *delta {
        return Err(RelationshipError::CorruptRecord(
            "noncanonical Relationship delta fields",
        ));
    }
    Ok(candidate)
}

fn apply_collection<T: Clone + Ord>(
    previous: &[T],
    change: &RelationshipCollectionChange<T>,
) -> Result<Vec<T>, RelationshipError> {
    change.validate_canonical()?;
    let mut candidate = previous.iter().cloned().collect::<BTreeSet<_>>();
    if candidate.len() != previous.len() {
        return Err(RelationshipError::CorruptRecord(
            "duplicate Relationship collection base",
        ));
    }
    for value in &change.removed {
        if !candidate.remove(value) {
            return Err(RelationshipError::CorruptRecord(
                "absent Relationship collection removal",
            ));
        }
    }
    for value in &change.added {
        if !candidate.insert(value.clone()) {
            return Err(RelationshipError::CorruptRecord(
                "existing Relationship collection addition",
            ));
        }
    }
    Ok(candidate.into_iter().collect())
}

#[cfg(test)]
#[path = "relationship_compact_apply_tests.rs"]
mod tests;
