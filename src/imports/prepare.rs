use super::*;
use crate::{IncomingAttachment, IncomingTurn};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct PreparedImport {
    pub receipt: ImportReceipt,
    pub turns: Vec<IncomingTurn>,
}
pub(crate) fn digest<T: serde::Serialize>(value: &T) -> Result<String, ImportError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn text(value: &str) -> Result<(), ImportError> {
    if value.is_empty() || value.len() > 4096 || value.contains('\0') {
        Err(ImportError::Invalid("empty, oversized or NUL source key"))
    } else {
        Ok(())
    }
}
pub(crate) fn prepare(input: &NativeImport) -> Result<PreparedImport, ImportError> {
    if input.schema_version != 1 {
        return Err(ImportError::Unsupported("native import schema"));
    }
    text(&input.source_namespace)?;
    text(&input.source_format)?;
    let mut canonical = input.clone();
    canonical.conversations.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen = BTreeSet::new();
    let mut count = 0usize;
    let mut byte_count = 0usize;
    for conversation in &mut canonical.conversations {
        text(&conversation.id)?;
        if !seen.insert(conversation.id.clone()) {
            return Err(ImportError::Invalid("duplicate conversation"));
        }
        conversation.messages.sort_by(|a, b| a.id.cmp(&b.id));
        conversation.branches.sort_by(|a, b| a.id.cmp(&b.id));
        count = count
            .checked_add(conversation.messages.len())
            .ok_or(ImportError::Invalid("size"))?;
        for message in &mut conversation.messages {
            message.artifacts.sort_by(|a, b| a.id.cmp(&b.id));
            byte_count = byte_count
                .checked_add(message.content.len())
                .ok_or(ImportError::Invalid("size"))?;
            for artifact in &message.artifacts {
                byte_count = byte_count
                    .checked_add(artifact.bytes.len())
                    .ok_or(ImportError::Invalid("size"))?;
            }
        }
    }
    if count > 100_000 || byte_count > 64 << 20 || canonical.conversations.len() > 10_000 {
        return Err(ImportError::Invalid("import exceeds bounds"));
    }
    let import_key = digest(&canonical)?;
    let mut receipt = ImportReceipt {
        schema_version: 1,
        import_key,
        source_namespace: input.source_namespace.clone(),
        source_format: input.source_format.clone(),
        conversations: Vec::new(),
    };
    let mut turns = Vec::new();
    for conversation in &canonical.conversations {
        let conversation_id = format!(
            "import-{}",
            digest(&(&input.source_namespace, &conversation.id))?
        );
        let messages: BTreeMap<_, _> = conversation
            .messages
            .iter()
            .map(|m| (m.id.clone(), m))
            .collect();
        if messages.len() != conversation.messages.len() {
            return Err(ImportError::Invalid("duplicate message"));
        }
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut ready = BTreeSet::new();
        for message in messages.values() {
            text(&message.id)?;
            if let Some(revision) = &message.revision {
                text(revision)?;
            }
            if let Some(replaces) = &message.replaces {
                text(replaces)?;
                if replaces == &message.id {
                    return Err(ImportError::Invalid("self replacement"));
                }
            }
            if !matches!(
                message.role.as_str(),
                "user" | "assistant" | "system" | "developer" | "tool"
            ) {
                return Err(ImportError::Unsupported("message role"));
            }
            if message.deleted && (!message.content.is_empty() || !message.artifacts.is_empty()) {
                return Err(ImportError::Invalid("deleted message carries a body"));
            }
            if let Some(parent) = &message.parent_id {
                if !messages.contains_key(parent) {
                    return Err(ImportError::Invalid("missing parent"));
                }
                children
                    .entry(parent.clone())
                    .or_default()
                    .push(message.id.clone());
            } else {
                ready.insert(message.id.clone());
            }
        }
        let mut resolved: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut imported = ImportedConversation {
            source_id: conversation.id.clone(),
            conversation_id: conversation_id.clone(),
            title: conversation.title.clone(),
            messages: Vec::new(),
            branches: Vec::new(),
        };
        while let Some(id) = ready.pop_first() {
            let message = messages[&id];
            let parent = message.parent_id.as_ref().and_then(|p| resolved[p].clone());
            let fingerprint = digest(message)?;
            let revision = message
                .revision
                .clone()
                .unwrap_or_else(|| fingerprint.clone());
            let node_id = if message.deleted {
                None
            } else {
                Some(format!(
                    "import-node-{}",
                    digest(&(&conversation_id, &id, &revision, &fingerprint, &parent))?
                ))
            };
            let mut artifacts = Vec::new();
            let mut attachments = Vec::new();
            let mut artifact_ids = BTreeSet::new();
            let mut file_ids = BTreeSet::new();
            for artifact in &message.artifacts {
                text(&artifact.id)?;
                text(&artifact.source_ref)?;
                if !artifact_ids.insert(&artifact.id) {
                    return Err(ImportError::Invalid("duplicate artifact key"));
                }
                let file = crate::file_store::build_stored_file(
                    artifact.filename.clone(),
                    artifact.mime_type.clone(),
                    &artifact.bytes,
                )?;
                // The native ID comes from Archive's existing file identity algorithm.
                let native_file_id = file
                    .id
                    .0
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                if !file_ids.insert(native_file_id.clone()) {
                    return Err(ImportError::Invalid("duplicate native artifact"));
                }
                artifacts.push(ImportedArtifact {
                    source_id: artifact.id.clone(),
                    source_ref: artifact.source_ref.clone(),
                    sha256: format!("{:x}", Sha256::digest(&artifact.bytes)),
                    file_id: native_file_id,
                });
                attachments.push(IncomingAttachment {
                    filename: artifact.filename.clone(),
                    mime_type: artifact.mime_type.clone(),
                    bytes: artifact.bytes.clone(),
                });
            }
            if let Some(node_id) = &node_id {
                turns.push(IncomingTurn {
                    id: node_id.clone(),
                    conversation_id: conversation_id.clone(),
                    parent_id: parent.clone(),
                    role: message.role.clone(),
                    principal_id: None,
                    timestamp_ns: message.timestamp_ns,
                    content: message.content.clone(),
                    attachments,
                    project_attachments: Vec::new(),
                });
            }
            // Tombstones omit this revision from the visible source path, preserving ancestry.
            resolved.insert(id.clone(), node_id.clone().or(parent));
            imported.messages.push(ImportedMessage {
                source_id: id.clone(),
                revision,
                source_parent_id: message.parent_id.clone(),
                replaces: message.replaces.clone(),
                deleted: message.deleted,
                node_id,
                fingerprint,
                artifacts,
            });
            if let Some(next) = children.get(&id) {
                ready.extend(next.iter().cloned());
            }
        }
        if resolved.len() != messages.len() {
            return Err(ImportError::Invalid("parent cycle"));
        }
        let mut branch_ids = BTreeSet::new();
        for branch in &conversation.branches {
            text(&branch.id)?;
            if !branch_ids.insert(&branch.id) {
                return Err(ImportError::Invalid("duplicate branch"));
            }
            let leaf = resolved
                .get(&branch.leaf_message_id)
                .ok_or(ImportError::Invalid("missing branch leaf"))?
                .clone();
            imported.branches.push(ImportedBranch {
                source_id: branch.id.clone(),
                branch_id: format!(
                    "import-branch-{}",
                    digest(&(&conversation_id, &branch.id, &leaf, branch.canonical))?
                ),
                leaf_node_id: leaf,
                canonical: branch.canonical,
            });
        }
        if !messages.is_empty() && imported.branches.is_empty() {
            return Err(ImportError::Invalid("messages require an explicit branch"));
        }
        let mut reachable = BTreeSet::new();
        for branch in &conversation.branches {
            let mut current = Some(branch.leaf_message_id.as_str());
            while let Some(id) = current {
                if !reachable.insert(id.to_owned()) {
                    break;
                }
                current = messages[id].parent_id.as_deref();
            }
        }
        if messages
            .values()
            .any(|m| !m.deleted && !reachable.contains(&m.id))
        {
            return Err(ImportError::Invalid(
                "source turn is outside supplied branches",
            ));
        }
        receipt.conversations.push(imported);
    }
    Ok(PreparedImport { receipt, turns })
}
