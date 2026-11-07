use crate::evidence_encoding::parse_hex;
use reliquary_owner_session::MemoryId;
use serde_json::{Map, Value};

pub(super) fn optional_hex_id(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<MemoryId>, String> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => parse_hex::<32>(value)
            .map(MemoryId)
            .map(Some)
            .ok_or_else(|| "memory_page_chain_id_invalid".into()),
        _ => Err("memory_page_chain_optional_id_invalid".into()),
    }
}

pub(super) fn optional_hex_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<[u8; 32]>, String> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => parse_hex::<32>(value)
            .map(Some)
            .ok_or_else(|| "memory_page_chain_digest_invalid".into()),
        _ => Err("memory_page_chain_optional_digest_invalid".into()),
    }
}

pub(super) fn require_true(report: &Value, field: &str) -> Result<(), String> {
    (report.get(field).and_then(Value::as_bool) == Some(true))
        .then_some(())
        .ok_or_else(|| format!("memory_page_chain_{field}_not_true"))
}

pub(super) fn object_at<'a>(
    report: &'a Value,
    path: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    path.iter()
        .try_fold(report, |value, part| value.get(*part))
        .and_then(Value::as_object)
        .ok_or_else(|| "memory_page_chain_report_shape_invalid".into())
}

pub(super) fn string_at<'a>(report: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    path.iter()
        .try_fold(report, |value, part| value.get(*part))
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_page_chain_report_field_invalid".into())
}

pub(super) fn string_at_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_page_chain_receipt_field_invalid".into())
}
