use super::providers::*;
use super::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// conversations.json mapping/text-v1 profile. No hidden state, network or filesystem reads.
pub fn normalize_chatgpt(namespace: &str, bytes: &[u8]) -> Result<NativeImport, ImportError> {
    let root = parse(bytes)?;
    let rows = root
        .as_array()
        .ok_or(ImportError::Unsupported("ChatGPT export array"))?;
    let mut conversations = Vec::new();
    for row in rows {
        let id = row
            .get("id")
            .or_else(|| row.get("conversation_id"))
            .and_then(Value::as_str)
            .ok_or(ImportError::Invalid("missing ChatGPT conversation id"))?;
        let mapping = row
            .get("mapping")
            .and_then(Value::as_object)
            .ok_or(ImportError::Unsupported("ChatGPT mapping"))?;
        let mut visible = BTreeMap::new();
        for (key, node) in mapping {
            let Some(message) = node.get("message").filter(|m| !m.is_null()) else {
                continue;
            };
            if message
                .pointer("/metadata/is_visually_hidden_from_conversation")
                .is_some_and(|v| !v.is_null() && !v.is_boolean())
            {
                return Err(ImportError::Invalid("ChatGPT hidden flag"));
            }
            if message
                .pointer("/metadata/is_visually_hidden_from_conversation")
                .and_then(Value::as_bool)
                == Some(true)
                || message
                    .get("channel")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s == "analysis")
            {
                continue;
            }
            if let Some(channel) = message.get("channel").filter(|v| !v.is_null()) {
                if !matches!(channel.as_str(), Some("final" | "commentary")) {
                    return Err(ImportError::Unsupported("ChatGPT channel"));
                }
            }
            if message
                .get("recipient")
                .filter(|v| !v.is_null())
                .is_some_and(|v| v.as_str() != Some("all"))
            {
                return Err(ImportError::Unsupported("ChatGPT tool recipient"));
            }
            let role = message
                .pointer("/author/role")
                .and_then(Value::as_str)
                .ok_or(ImportError::Invalid("ChatGPT role"))?;
            if !matches!(role, "user" | "assistant" | "system" | "developer") {
                return Err(ImportError::Unsupported("ChatGPT role"));
            }
            if string(message, "id")? != key {
                return Err(ImportError::Invalid("mapping/message id mismatch"));
            }
            if message
                .pointer("/metadata/attachments")
                .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
            {
                return Err(ImportError::Unsupported(
                    "ChatGPT attachment bytes require normalized artifacts",
                ));
            }
            let content = message
                .get("content")
                .ok_or(ImportError::Invalid("ChatGPT content"))?;
            if string(content, "content_type")? != "text" {
                return Err(ImportError::Unsupported("ChatGPT multimodal content"));
            }
            let text = array(content, "parts")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or(ImportError::Unsupported("ChatGPT non-text part"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("\n");
            visible.insert(
                key.clone(),
                ImportMessage {
                    id: key.clone(),
                    revision: None,
                    parent_id: None,
                    replaces: None,
                    deleted: false,
                    role: role.to_owned(),
                    timestamp_ns: seconds(
                        message
                            .get("create_time")
                            .ok_or(ImportError::Invalid("ChatGPT timestamp"))?,
                    )?,
                    content: text,
                    artifacts: Vec::new(),
                },
            );
        }
        let visible_ids: BTreeSet<_> = visible.keys().cloned().collect();
        for (key, message) in &mut visible {
            let mut parent = optional(&mapping[key], "parent")?;
            let mut seen = BTreeSet::new();
            while let Some(p) = parent.clone() {
                if !seen.insert(p.clone()) {
                    return Err(ImportError::Invalid("ChatGPT parent cycle"));
                }
                if visible_ids.contains(&p) {
                    break;
                }
                let node = mapping
                    .get(&p)
                    .ok_or(ImportError::Invalid("ChatGPT missing parent"))?;
                parent = optional(node, "parent")?;
            }
            message.parent_id = parent;
        }
        let messages: Vec<_> = visible.into_values().collect();
        // Export selection may reference a filtered root/hidden leaf: reject rather than guess.
        let selected = optional(row, "current_node")?;
        let branch_list = branches(&messages, selected.as_deref())?;
        conversations.push(ImportConversation {
            id: id.to_owned(),
            title: optional(row, "title")?,
            messages,
            branches: branch_list,
        });
    }
    result(namespace, "chatgpt-mapping-text-v1", conversations)
}
