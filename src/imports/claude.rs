use super::providers::*;
use super::*;
use serde_json::Value;

/// conversations.json uuid/chat_messages/text-v1 profile; supplied UUID ancestry is retained.
pub fn normalize_claude(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    let root = parse(bytes)?;
    let rows = root
        .as_array()
        .ok_or(ImportError::Unsupported("Claude export array"))?;
    let mut conversations = Vec::new();
    for row in rows {
        let mut messages = Vec::new();
        let mut previous = None;
        for message in array(row, "chat_messages")? {
            if ["attachments", "files"].iter().any(|key| {
                message
                    .get(key)
                    .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
            }) {
                return Err(ImportError::Unsupported(
                    "Claude attachment bytes require normalized artifacts",
                ));
            }
            let role = match string(message, "sender")? {
                "human" => "user",
                "assistant" => "assistant",
                _ => return Err(ImportError::Unsupported("Claude sender")),
            };
            if message
                .get("content")
                .is_some_and(|v| !v.is_null() && !v.is_array())
            {
                return Err(ImportError::Unsupported("Claude content shape"));
            }
            let content = if let Some(blocks) = message
                .get("content")
                .and_then(Value::as_array)
                .filter(|b| !b.is_empty())
            {
                let mut texts = Vec::new();
                for block in blocks {
                    match string(block, "type")? {
                        "text" => texts.push(
                            block
                                .get("text")
                                .and_then(Value::as_str)
                                .ok_or(ImportError::Invalid("Claude text block"))?,
                        ),
                        "thinking" | "redacted_thinking" => {}
                        _ => return Err(ImportError::Unsupported("Claude non-text block")),
                    }
                }
                texts.join("\n")
            } else {
                message
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(ImportError::Invalid("Claude text"))?
                    .to_owned()
            };
            let id = string(message, "uuid")?.to_owned();
            let parent_id = if message.get("parent_message_uuid").is_some() {
                optional(message, "parent_message_uuid")?
            } else {
                previous.clone()
            };
            messages.push(ImportMessage {
                id: id.clone(),
                revision: None,
                parent_id,
                replaces: None,
                deleted: false,
                role: role.to_owned(),
                timestamp_ns: iso(message
                    .get("created_at")
                    .ok_or(ImportError::Invalid("Claude timestamp"))?)?,
                content,
                artifacts: Vec::new(),
            });
            previous = Some(id);
        }
        let branch_list = branches(&messages, None)?;
        conversations.push(ImportConversation {
            id: string(row, "uuid")?.to_owned(),
            title: optional(row, "name")?,
            messages,
            branches: branch_list,
        });
    }
    result(namespace, "claude-uuid-text-v1", conversations)
}
