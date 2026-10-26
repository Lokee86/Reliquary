//! Pure application of an accepted Memory delta to its prior revision.
use crate::MemoryError;
use crate::memory_compact_contract::MemoryDelta;
use crate::memory_model::MemoryRecord;
use reliquary_container::codec::apply_field;

pub(crate) fn apply_delta(
    previous: &MemoryRecord,
    delta: &MemoryDelta,
) -> Result<MemoryRecord, MemoryError> {
    if delta.id != previous.id {
        return Err(MemoryError::SemanticMutation);
    }
    if delta.previous_revision != previous.revision
        || previous.revision.checked_add(1) != Some(delta.revision)
    {
        return Err(MemoryError::RevisionConflict);
    }

    // Spell out every field so a new MemoryRecord field requires an explicit
    // decision here. The body reference and stable ID are inherited/verified;
    // the remaining envelope fields come from the delta.
    let candidate = MemoryRecord {
        id: previous.id,
        revision: delta.revision,
        body_id: previous.body_id,
        category: apply_field(&previous.category, &delta.fields.category),
        memory_type: apply_field(&previous.memory_type, &delta.fields.memory_type),
        authority_kind: apply_field(&previous.authority_kind, &delta.fields.authority_kind),
        temporal_status: apply_field(&previous.temporal_status, &delta.fields.temporal_status),
        scope: apply_field(&previous.scope, &delta.fields.scope),
        lifecycle_state: apply_field(&previous.lifecycle_state, &delta.fields.lifecycle_state),
        archived: apply_field(&previous.archived, &delta.fields.archived),
        superseded_by: apply_field(&previous.superseded_by, &delta.fields.superseded_by),
        parent_id: apply_field(&previous.parent_id, &delta.fields.parent_id),
        source_node_id: apply_field(&previous.source_node_id, &delta.fields.source_node_id),
        content_source_conversation_id: apply_field(
            &previous.content_source_conversation_id,
            &delta.fields.content_source_conversation_id,
        ),
        content_source_node_id: apply_field(
            &previous.content_source_node_id,
            &delta.fields.content_source_node_id,
        ),
        grounding_source_conversation_id: apply_field(
            &previous.grounding_source_conversation_id,
            &delta.fields.grounding_source_conversation_id,
        ),
        grounding_source_node_id: apply_field(
            &previous.grounding_source_node_id,
            &delta.fields.grounding_source_node_id,
        ),
        source_episode_id: apply_field(
            &previous.source_episode_id,
            &delta.fields.source_episode_id,
        ),
        source_time_ns: apply_field(&previous.source_time_ns, &delta.fields.source_time_ns),
        source_ref: apply_field(&previous.source_ref, &delta.fields.source_ref),
        temporal_inference: apply_field(
            &previous.temporal_inference,
            &delta.fields.temporal_inference,
        ),
        mutation_id: delta.mutation_id.clone(),
        created_at_ns: apply_field(&previous.created_at_ns, &delta.fields.created_at_ns),
        updated_at_ns: apply_field(&previous.updated_at_ns, &delta.fields.updated_at_ns),
        global_version: delta.global_version,
        memory_version: delta.memory_version,
    };

    // Reuse the contract's envelope validation and derive the unique canonical
    // representation. This also rejects redundant Set(value-equal-to-base).
    let canonical = MemoryDelta::between(previous, &candidate)?;
    if canonical != *delta {
        return Err(MemoryError::CorruptRecord(
            "noncanonical Memory delta fields",
        ));
    }
    Ok(candidate)
}

#[cfg(test)]
#[path = "memory_compact_apply_tests.rs"]
mod tests;
