//! Pure Entity candidate reconstruction; store admission owns publication authority.
use crate::EntityError;
use crate::compact_delta::apply_field;
use crate::entity_compact_contract::EntityDelta;
use crate::entity_model::EntityRecord;

pub(crate) fn apply_delta(
    previous: &EntityRecord,
    delta: &EntityDelta,
) -> Result<EntityRecord, EntityError> {
    if previous.id != delta.id {
        return Err(EntityError::InvalidField("Entity delta identity"));
    }
    if delta.previous_revision != previous.revision
        || previous.revision.checked_add(1) != Some(delta.revision)
    {
        return Err(EntityError::RevisionConflict);
    }
    let candidate = EntityRecord {
        id: previous.id,
        global_id: apply_field(&previous.global_id, &delta.fields.global_id),
        revision: delta.revision,
        canonical_name: apply_field(&previous.canonical_name, &delta.fields.canonical_name),
        aliases: apply_field(&previous.aliases, &delta.fields.aliases),
        kind: apply_field(&previous.kind, &delta.fields.kind),
        summary: apply_field(&previous.summary, &delta.fields.summary),
        mutation_id: delta.mutation_id.clone(),
        created_at_ns: previous.created_at_ns,
        updated_at_ns: apply_field(&previous.updated_at_ns, &delta.fields.updated_at_ns),
        global_version: delta.global_version,
        entity_version: delta.entity_version,
    };
    let canonical = match &delta.description_proof {
        Some(proof) => EntityDelta::between_pruned(previous, &candidate, proof.clone())?,
        None => EntityDelta::between(previous, &candidate)?,
    };
    if canonical != *delta {
        return Err(EntityError::CorruptRecord(
            "noncanonical Entity delta fields",
        ));
    }
    Ok(candidate)
}

#[cfg(test)]
#[path = "entity_compact_apply_tests.rs"]
mod tests;
