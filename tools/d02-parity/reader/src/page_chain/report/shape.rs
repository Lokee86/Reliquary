use super::super::receipt::PAGE_CHAIN_SCHEMA_ID;
use crate::cursor::{CURSOR_FAMILY, CURSOR_ORDER};
use reliquary_owner_session::{
    MAX_MEMORY_ID_CURSOR_BYTES, MAX_MEMORY_ID_PAGE_COUNT, MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES,
};
use serde_json::{Map, Value};
use std::io::{self, Write};

const PAGE_FIELDS: [&str; 14] = [
    "schema_id",
    "schema_sha256",
    "page_index",
    "history_generation_sha256",
    "memory_version",
    "family",
    "order",
    "after_id_exclusive",
    "last_id",
    "returned_count",
    "raw_payload_bytes",
    "previous_page_digest",
    "page_digest",
    "terminal_empty",
];

const MEMORY_FIELDS: [&str; 16] = [
    "family",
    "status",
    "coverage",
    "memory_version",
    "ids",
    "returned_count",
    "raw_payload_bytes",
    "maximum_items",
    "maximum_raw_payload_bytes",
    "cursor_maximum_bytes",
    "next_cursor",
    "page_chain",
    "revision_history",
    "bodies",
    "lifecycle_and_provenance",
    "serialized_page_bytes",
];

const MAX_MEMORY_PAGE_JSON_BYTES: usize = 10_240;

pub(super) fn validate_memory_page_shape(memory: &Map<String, Value>) -> Result<(), String> {
    if memory.len() != MEMORY_FIELDS.len()
        || MEMORY_FIELDS
            .iter()
            .any(|field| !memory.contains_key(*field))
    {
        return Err("memory_page_chain_memory_observation_schema_mismatch".into());
    }

    let ids = memory
        .get("ids")
        .and_then(Value::as_array)
        .ok_or("memory_page_chain_ids_missing")?;
    if ids.len() > MAX_MEMORY_ID_PAGE_COUNT
        || ids
            .iter()
            .any(|value| value.as_str().is_none_or(|id| id.len() != 64))
    {
        return Err("memory_page_chain_ids_exceed_or_miss_page_bound".into());
    }

    let chain = memory
        .get("page_chain")
        .and_then(Value::as_object)
        .ok_or("memory_page_chain_receipt_missing")?;
    if chain.len() != PAGE_FIELDS.len()
        || PAGE_FIELDS.iter().any(|field| !chain.contains_key(*field))
    {
        return Err("memory_page_chain_receipt_schema_mismatch".into());
    }

    if bounded_string(memory, "family", CURSOR_FAMILY.len())? != CURSOR_FAMILY
        || bounded_string(memory, "status", 10)? != "INCOMPLETE"
        || bounded_string(memory, "coverage", 64)? != "one_current_memory_identity_page_only"
        || bounded_string(memory, "maximum_items", 3)? != MAX_MEMORY_ID_PAGE_COUNT.to_string()
        || bounded_string(memory, "maximum_raw_payload_bytes", 4)?
            != MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES.to_string()
        || bounded_string(memory, "cursor_maximum_bytes", 3)?
            != MAX_MEMORY_ID_CURSOR_BYTES.to_string()
        || bounded_string(memory, "revision_history", 10)? != "INCOMPLETE"
        || bounded_string(memory, "bodies", 10)? != "INCOMPLETE"
        || bounded_string(memory, "lifecycle_and_provenance", 10)? != "INCOMPLETE"
    {
        return Err("memory_page_chain_memory_observation_mismatch".into());
    }
    let _ = bounded_string(memory, "memory_version", 20)?;
    let _ = bounded_string(memory, "returned_count", 3)?;
    let _ = bounded_string(memory, "raw_payload_bytes", 4)?;
    let _ = bounded_string(memory, "serialized_page_bytes", 5)?;
    match memory.get("next_cursor") {
        Some(Value::Null) => {}
        Some(Value::String(cursor)) if cursor.len() <= MAX_MEMORY_ID_CURSOR_BYTES => {}
        _ => return Err("memory_page_chain_cursor_shape_invalid".into()),
    }

    let _ = bounded_string(chain, "schema_id", PAGE_CHAIN_SCHEMA_ID.len())?;
    let _ = bounded_string(chain, "schema_sha256", 64)?;
    let _ = bounded_string(chain, "page_index", 20)?;
    let _ = bounded_string(chain, "history_generation_sha256", 64)?;
    let _ = bounded_string(chain, "memory_version", 20)?;
    let _ = bounded_string(chain, "family", CURSOR_FAMILY.len())?;
    let _ = bounded_string(chain, "order", CURSOR_ORDER.len())?;
    let _ = bounded_optional_string(chain, "after_id_exclusive", 64)?;
    let _ = bounded_optional_string(chain, "last_id", 64)?;
    let _ = bounded_string(chain, "returned_count", 3)?;
    let _ = bounded_string(chain, "raw_payload_bytes", 4)?;
    let _ = bounded_optional_string(chain, "previous_page_digest", 64)?;
    let _ = bounded_string(chain, "page_digest", 64)?;
    if chain
        .get("terminal_empty")
        .and_then(Value::as_bool)
        .is_none()
    {
        return Err("memory_page_chain_terminal_marker_invalid".into());
    }

    validate_serialized_page_size(memory)
}

fn bounded_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum_bytes: usize,
) -> Result<&'a str, String> {
    match object.get(field).and_then(Value::as_str) {
        Some(value) if value.len() <= maximum_bytes => Ok(value),
        _ => Err("memory_page_chain_bounded_string_invalid".into()),
    }
}

fn bounded_optional_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum_bytes: usize,
) -> Result<Option<&'a str>, String> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.len() <= maximum_bytes => Ok(Some(value)),
        _ => Err("memory_page_chain_bounded_optional_string_invalid".into()),
    }
}

fn validate_serialized_page_size(memory: &Map<String, Value>) -> Result<(), String> {
    let reported = bounded_string(memory, "serialized_page_bytes", 5)?
        .parse::<usize>()
        .map_err(|_| "memory_page_chain_serialized_size_invalid")?;
    let actual = serialized_page_size(memory)?;
    if actual != reported {
        return Err("memory_page_chain_serialized_size_mismatch".into());
    }
    Ok(())
}

fn serialized_page_size(memory: &Map<String, Value>) -> Result<usize, String> {
    let mut writer = CappedCounter { written: 0 };
    serde_json::to_writer(&mut writer, memory)
        .map_err(|_| "memory_page_chain_serialized_size_exceeds_bound")?;
    Ok(writer.written)
}

struct CappedCounter {
    written: usize,
}

impl Write for CappedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MEMORY_PAGE_JSON_BYTES.saturating_sub(self.written) {
            return Err(io::Error::other("serialized page exceeds byte limit"));
        }
        self.written += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capped_serializer_rejects_nested_oversized_json_without_buffering_it() {
        let memory = Map::from_iter([(
            "nested".to_string(),
            json!({"payload": "x".repeat(MAX_MEMORY_PAGE_JSON_BYTES * 4)}),
        )]);
        assert_eq!(
            serialized_page_size(&memory).unwrap_err(),
            "memory_page_chain_serialized_size_exceeds_bound"
        );
    }
}
