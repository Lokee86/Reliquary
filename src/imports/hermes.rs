use super::providers::{array, branches, optional, result, seconds, string};
use super::*;
use serde_json::Value;

/// Hermes full-session JSONL text profile, audited at 7798df0a83.
/// Active text rows with exported database IDs only. No DB access or inferred deletion.
pub fn normalize_hermes_jsonl(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    if bytes.len() > 64 << 20 {
        return Err(ImportError::Invalid("export exceeds bounds"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ImportError::Invalid("export UTF-8"))?;
    let mut conversations = Vec::new();
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        let row: Value = serde_json::from_str(line)?;
        if optional(&row, "parent_session_id")?.is_some() || row.get("lineage_segments").is_some() {
            return Err(ImportError::Unsupported(
                "Hermes compression/fork lineage requires normalized ancestry",
            ));
        }
        let mut messages = Vec::new();
        let mut previous = None;
        for message in array(&row, "messages")? {
            if message.get("_compressed_summary").and_then(Value::as_bool) == Some(true)
                || message
                    .get("active")
                    .is_some_and(|v| v != &serde_json::json!(1) && v != &serde_json::json!(true))
            {
                return Err(ImportError::Unsupported("Hermes inactive/compacted row"));
            }
            if message
                .get("tool_calls")
                .is_some_and(|v| !v.is_null() && v != &serde_json::json!([]))
                || message.get("tool_call_id").is_some_and(|v| !v.is_null())
            {
                return Err(ImportError::Unsupported(
                    "Hermes tool provenance requires normalized source events",
                ));
            }
            let id = match message.get("id") {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                Some(Value::Number(n)) if n.as_u64().is_some() => n.to_string(),
                _ => {
                    return Err(ImportError::Unsupported(
                        "Hermes export missing stable message ID",
                    ));
                }
            };
            let content = message
                .get("content")
                .and_then(Value::as_str)
                .ok_or(ImportError::Unsupported("Hermes non-text content"))?
                .to_owned();
            messages.push(ImportMessage {
                id: id.clone(),
                revision: None,
                parent_id: previous.clone(),
                replaces: None,
                deleted: false,
                role: string(message, "role")?.to_owned(),
                timestamp_ns: seconds(
                    message
                        .get("timestamp")
                        .ok_or(ImportError::Invalid("Hermes timestamp"))?,
                )?,
                content,
                artifacts: Vec::new(),
            });
            previous = Some(id);
        }
        let branch_list = branches(&messages, None)?;
        conversations.push(ImportConversation {
            id: string(&row, "id")?.to_owned(),
            title: optional(&row, "title")?,
            messages,
            branches: branch_list,
        });
    }
    result(namespace, "hermes-session-jsonl-text-v1", conversations)
}
