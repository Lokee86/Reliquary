use super::*;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn parse(bytes: &[u8]) -> Result<Value, ImportError> {
    if bytes.len() > 64 << 20 {
        return Err(ImportError::Invalid("export exceeds bounds"));
    }
    Ok(serde_json::from_slice(bytes)?)
}
pub(super) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, ImportError> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(ImportError::Invalid("missing export string"))
}
pub(super) fn optional(v: &Value, key: &str) -> Result<Option<String>, ImportError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(ImportError::Invalid("invalid optional export string")),
    }
}
pub(super) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, ImportError> {
    v.get(key)
        .and_then(Value::as_array)
        .ok_or(ImportError::Invalid("missing export array"))
}
pub(super) fn seconds(v: &Value) -> Result<i64, ImportError> {
    let n = v
        .as_f64()
        .ok_or(ImportError::Invalid("missing source timestamp"))?
        * 1_000_000_000.0;
    if !n.is_finite() || n < i64::MIN as f64 || n >= i64::MAX as f64 {
        return Err(ImportError::Invalid("source timestamp overflow"));
    }
    Ok(n.round() as i64)
}
pub(super) fn iso(v: &Value) -> Result<i64, ImportError> {
    let t = time::OffsetDateTime::parse(
        v.as_str()
            .ok_or(ImportError::Invalid("missing source timestamp"))?,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| ImportError::Invalid("invalid source timestamp"))?;
    i64::try_from(t.unix_timestamp_nanos())
        .map_err(|_| ImportError::Invalid("source timestamp overflow"))
}
pub(super) fn branches(
    messages: &[ImportMessage],
    selected: Option<&str>,
) -> Result<Vec<ImportBranch>, ImportError> {
    let parents: BTreeSet<_> = messages
        .iter()
        .filter_map(|m| m.parent_id.as_deref())
        .collect();
    let leaves: Vec<_> = messages
        .iter()
        .filter(|m| !parents.contains(m.id.as_str()))
        .collect();
    if selected.is_some_and(|s| !leaves.iter().any(|m| m.id == s)) {
        return Err(ImportError::Unsupported(
            "selected node is not a visible leaf",
        ));
    }
    Ok(leaves
        .into_iter()
        .map(|m| ImportBranch {
            id: m.id.clone(),
            leaf_message_id: m.id.clone(),
            canonical: selected.map_or(true, |s| s == m.id),
        })
        .collect())
}
pub(super) fn result(
    namespace: &str,
    format: &str,
    conversations: Vec<ImportConversation>,
) -> Result<NativeImport, ImportError> {
    let value = NativeImport {
        schema_version: 1,
        source_namespace: namespace.to_owned(),
        source_format: format.to_owned(),
        conversations,
    };
    prepare(&value)?;
    Ok(value)
}

/// Provider-neutral structured agent interchange, schema 1. The producer supplies stable
/// message/branch/revision/artifact facts. This does not claim Codex/ACP/Hermes raw-log support.
pub fn normalize_agent(bytes: &[u8]) -> Result<NativeImport, ImportError> {
    if bytes.len() > 64 << 20 {
        return Err(ImportError::Invalid("export exceeds bounds"));
    }
    let input: NativeImport = serde_json::from_slice(bytes)?;
    prepare(&input)?;
    Ok(input)
}
