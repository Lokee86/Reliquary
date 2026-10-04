use super::owner_operations::FrozenPreparedMemory;
use crate::insomnia::PreparedMemory;
use crate::memory_model::{self, MemoryRecord};
use crate::{
    Memory, MemoryBodyId, MemoryDraft, MemoryId, MemoryRoutingMetadata, MemorySourceRef,
    MemoryTemporalInference,
};
use serde_json::{Map, Value, json};

type HostError = super::ReliquaryRuntimeHostError;
type Result<T> = std::result::Result<T, HostError>;

/// Freezes the exact PreparedMemory data needed to resume publication after restart.
///
/// The journal JSON is a strict envelope; semantic record bytes remain owned by the
/// canonical Memory codec, and routing bytes remain owned by memory_routing_codec.
pub(crate) fn freeze_memory(
    prepared: &PreparedMemory,
    source_ref: Option<&MemorySourceRef>,
) -> Result<FrozenPreparedMemory> {
    let draft = &prepared.draft;
    let id = memory_model::memory_id(&draft.mutation_id);
    let body_id = memory_model::memory_body_id(&draft.title, &draft.content);
    let record = MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: draft.category.clone(),
        memory_type: draft.memory_type.clone(),
        authority_kind: draft.authority_kind.clone(),
        temporal_status: draft.temporal_status.clone(),
        scope: draft.scope.clone(),
        lifecycle_state: draft.lifecycle_state.clone(),
        archived: draft.archived,
        superseded_by: draft.superseded_by,
        parent_id: draft.parent_id,
        source_node_id: draft.source_node_id.clone(),
        content_source_conversation_id: draft.content_source_conversation_id.clone(),
        content_source_node_id: draft.content_source_node_id.clone(),
        grounding_source_conversation_id: draft.grounding_source_conversation_id.clone(),
        grounding_source_node_id: draft.grounding_source_node_id.clone(),
        source_episode_id: draft.source_episode_id,
        source_time_ns: draft.source_time_ns,
        source_ref: source_ref.cloned(),
        temporal_inference: prepared.temporal_inference.clone().map(|inference| {
            MemoryTemporalInference {
                body_id,
                source_time_ns: draft.source_time_ns,
                inference,
            }
        }),
        mutation_id: draft.mutation_id.clone(),
        created_at_ns: draft.created_at_ns,
        updated_at_ns: draft.updated_at_ns,
        global_version: 0,
        memory_version: 0,
    };
    let record_bytes = crate::memory_codec::encode_record(&record).map_err(operation)?;
    let routing_bytes = prepared
        .routing_metadata
        .as_ref()
        .map(crate::memory_routing_codec::encode)
        .transpose()
        .map_err(operation)?;
    if let Some(routing) = &prepared.routing_metadata {
        validate_routing(routing, id, body_id)?;
    }
    let mut envelope = Map::new();
    envelope.insert("title".into(), Value::String(draft.title.clone()));
    envelope.insert("content".into(), Value::String(draft.content.clone()));
    envelope.insert("record".into(), json!(record_bytes));
    envelope.insert(
        "routing".into(),
        routing_bytes.map_or(Value::Null, |bytes| json!(bytes)),
    );
    Ok(FrozenPreparedMemory {
        prepared_memory: Value::Object(envelope),
    })
}

/// Thaws a frozen value only when the complete envelope and semantic bindings validate.
pub(crate) fn thaw_memory(
    frozen: &FrozenPreparedMemory,
) -> Result<(PreparedMemory, Option<MemorySourceRef>)> {
    let envelope = frozen
        .prepared_memory
        .as_object()
        .ok_or_else(|| invalid("frozen memory envelope must be an object"))?;
    if envelope.len() != 4
        || !["title", "content", "record", "routing"]
            .iter()
            .all(|key| envelope.contains_key(*key))
    {
        return Err(invalid(
            "frozen memory envelope has missing or unknown fields",
        ));
    }
    let title = envelope
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("frozen memory title must be a string"))?;
    let content = envelope
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("frozen memory content must be a string"))?;
    let record_bytes = bytes_field(
        envelope
            .get("record")
            .ok_or_else(|| invalid("missing frozen memory record"))?,
    )?;
    let record = crate::memory_codec::decode_record(&record_bytes)
        .map_err(operation)?
        .ok_or_else(|| invalid("frozen memory record has an unknown format"))?;
    if record.revision != 1 {
        return Err(invalid("frozen memory revision must be one"));
    }
    let expected_id = memory_model::memory_id(&record.mutation_id);
    let expected_body = memory_model::memory_body_id(title, content);
    if record.id != expected_id || record.body_id != expected_body {
        return Err(invalid("frozen memory identity or body binding mismatch"));
    }
    if let Some(temporal) = &record.temporal_inference {
        if temporal.body_id != expected_body || temporal.source_time_ns != record.source_time_ns {
            return Err(invalid("frozen temporal inference binding mismatch"));
        }
    }
    let routing_value = envelope.get("routing").unwrap();
    let routing_metadata = match routing_value {
        Value::Null => None,
        value => {
            let bytes = bytes_field(value)?;
            let metadata = crate::memory_routing_codec::decode(&bytes)
                .map_err(operation)?
                .ok_or_else(|| invalid("frozen routing metadata has an unknown format"))?;
            validate_routing(&metadata, expected_id, expected_body)?;
            Some(metadata)
        }
    };
    let draft = MemoryDraft {
        category: record.category.clone(),
        memory_type: record.memory_type.clone(),
        authority_kind: record.authority_kind.clone(),
        temporal_status: record.temporal_status.clone(),
        title: title.to_owned(),
        content: content.to_owned(),
        scope: record.scope.clone(),
        lifecycle_state: record.lifecycle_state.clone(),
        archived: record.archived,
        superseded_by: record.superseded_by,
        parent_id: record.parent_id,
        source_node_id: record.source_node_id.clone(),
        content_source_conversation_id: record.content_source_conversation_id.clone(),
        content_source_node_id: record.content_source_node_id.clone(),
        grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
        grounding_source_node_id: record.grounding_source_node_id.clone(),
        source_episode_id: record.source_episode_id,
        source_time_ns: record.source_time_ns,
        mutation_id: record.mutation_id.clone(),
        created_at_ns: record.created_at_ns,
        updated_at_ns: record.updated_at_ns,
    };
    let temporal = crate::insomnia::temporal::assess_draft(&draft);
    Ok((
        PreparedMemory {
            draft,
            temporal,
            temporal_inference: record.temporal_inference.map(|value| value.inference),
            routing_metadata,
        },
        record.source_ref,
    ))
}

/// Confirms a PHY write is the original revision-one publication represented by a freeze.
pub(crate) fn verify_memory(frozen: &FrozenPreparedMemory, memory: &Memory) -> Result<()> {
    verify_memory_record(frozen, memory)?;
    let (prepared, _) = thaw_memory(frozen)?;
    if memory.routing_metadata != prepared.routing_metadata {
        return Err(invalid(
            "published routing metadata differs from frozen draft",
        ));
    }
    Ok(())
}

/// Verify semantic publication before installing its separate routing attachment.
pub(crate) fn verify_memory_record(frozen: &FrozenPreparedMemory, memory: &Memory) -> Result<()> {
    let (prepared, _) = thaw_memory(frozen)?;
    if memory.revision != 1 {
        return Err(invalid("published memory revision is not one"));
    }
    let expected_id = memory_model::memory_id(&prepared.draft.mutation_id);
    let expected_body =
        memory_model::memory_body_id(&prepared.draft.title, &prepared.draft.content);
    if memory.id != expected_id
        || memory.title != prepared.draft.title
        || memory.content != prepared.draft.content
    {
        return Err(invalid(
            "published memory identity or body differs from frozen draft",
        ));
    }
    let envelope = frozen
        .prepared_memory
        .as_object()
        .ok_or_else(|| invalid("frozen memory envelope must be an object"))?;
    let expected_bytes = bytes_field(
        envelope
            .get("record")
            .ok_or_else(|| invalid("missing frozen memory record"))?,
    )?;
    let actual = record_from_memory(
        memory,
        memory.source_ref.clone(),
        memory_model::memory_body_id(&memory.title, &memory.content),
    )?;
    let actual_bytes = crate::memory_codec::encode_record(&actual).map_err(operation)?;
    if expected_bytes != actual_bytes {
        return Err(invalid(
            "published memory metadata differs from frozen draft",
        ));
    }
    if let Some(routing) = &memory.routing_metadata {
        validate_routing(routing, expected_id, expected_body)?;
    }
    Ok(())
}

fn record_from_memory(
    memory: &Memory,
    source_ref: Option<MemorySourceRef>,
    body_id: MemoryBodyId,
) -> Result<MemoryRecord> {
    Ok(MemoryRecord {
        id: memory.id,
        revision: memory.revision,
        body_id,
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: memory.lifecycle_state.clone(),
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id.clone(),
        content_source_conversation_id: memory.content_source_conversation_id.clone(),
        content_source_node_id: memory.content_source_node_id.clone(),
        grounding_source_conversation_id: memory.grounding_source_conversation_id.clone(),
        grounding_source_node_id: memory.grounding_source_node_id.clone(),
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        source_ref,
        temporal_inference: memory.temporal_inference.clone(),
        mutation_id: memory.mutation_id.clone(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns,
        global_version: 0,
        memory_version: 0,
    })
}

fn validate_routing(
    metadata: &MemoryRoutingMetadata,
    id: MemoryId,
    body_id: MemoryBodyId,
) -> Result<()> {
    if metadata.memory_id != id || metadata.body_id != body_id {
        return Err(invalid(
            "frozen routing metadata identity or body binding mismatch",
        ));
    }
    Ok(())
}

fn bytes_field(value: &Value) -> Result<Vec<u8>> {
    value
        .as_array()
        .ok_or_else(|| invalid("frozen codec bytes must be an array"))?
        .iter()
        .map(|byte| {
            byte.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| invalid("frozen codec byte is outside u8 range"))
        })
        .collect()
}

fn invalid(message: &'static str) -> HostError {
    HostError::Operation(message.into())
}

fn operation(error: impl std::fmt::Display) -> HostError {
    HostError::Operation(error.to_string())
}

#[cfg(test)]
#[path = "runtime_host_owner_publication_codec_tests.rs"]
mod tests;
