use super::super::*;
use reliquary_archive::{ArchiveError, Branch, EpisodeId};

impl RelSession {
    pub fn management_source_turn(
        &self,
        conversation: &str,
        node: &str,
        max_bytes: usize,
        include_import_reference: bool,
    ) -> Result<Option<RelSourceTurn>, OwnerSessionError> {
        let Some(record) = self.archive.node(conversation, node) else {
            return Ok(None);
        };
        let terminal = self.archive.conversation_tombstone(conversation).is_some();
        let content = self.archive.content_for_node(conversation, node);
        if content.is_some_and(|text| text.len() > max_bytes) && !terminal {
            return Err(OwnerSessionError::ManagementLimit);
        }
        let imported = if include_import_reference {
            self.management_imported_source(conversation, node)?
        } else {
            None
        };
        Ok(Some(RelSourceTurn {
            parent_node_id: record.parent_id.clone(),
            node_id: record.id.clone(),
            role: record.role.clone(),
            principal_id: record.principal_id.clone(),
            timestamp_ns: record.timestamp_ns,
            content_id: record.content_id.0,
            archive_revision: self.archive.node_archive_version(conversation, node),
            content: if terminal {
                None
            } else {
                content.map(str::to_owned)
            },
            terminal,
            imported,
        }))
    }

    fn management_imported_source(
        &self,
        conversation: &str,
        node: &str,
    ) -> Result<Option<RelImportedSource>, OwnerSessionError> {
        let Some((receipt, conversation, message)) =
            self.archive.imported_turn_reference(conversation, node)
        else {
            return Ok(None);
        };
        let fields = [
            receipt.source_namespace.as_str(),
            receipt.source_format.as_str(),
            conversation.source_id.as_str(),
            message.source_id.as_str(),
            message.revision.as_str(),
            message.source_parent_id.as_deref().unwrap_or_default(),
        ];
        if fields.iter().any(|field| field.len() > 4096) {
            return Err(OwnerSessionError::ManagementLimit);
        }
        Ok(Some(RelImportedSource {
            namespace: receipt.source_namespace.clone(),
            format: receipt.source_format.clone(),
            source_digest: receipt.source_digest,
            receipt_digest: receipt.receipt_digest,
            conversation_id: conversation.source_id.clone(),
            message_id: message.source_id.clone(),
            message_revision: message.revision.clone(),
            parent_message_id: message.source_parent_id.clone(),
            source_author: message.source_author.clone(),
            source_branch_id: (receipt.source_format
                == reliquary_archive::HERMES_COPIED_SOURCE_FORMAT)
                .then(|| conversation.branches.first())
                .flatten()
                .map(|branch| branch.source_id.clone()),
        }))
    }

    pub fn management_episode_source(
        &self,
        episode: [u8; 32],
        source_node: &str,
    ) -> Result<Option<&str>, OwnerSessionError> {
        let Some(episode) = self.archive.episode(EpisodeId(episode)) else {
            return Ok(None);
        };
        let mut current = Some(episode.end_node_id.as_str());
        while let Some(id) = current {
            if id == source_node {
                return Ok(Some(&episode.conversation_id));
            }
            if id == episode.start_node_id {
                break;
            }
            current = self
                .archive
                .node(&episode.conversation_id, id)
                .and_then(|node| node.parent_id.as_deref());
        }
        Err(OwnerSessionError::Record(
            "Memory source node does not belong to its accepted Episode".into(),
        ))
    }

    pub fn management_conversations(&self) -> impl Iterator<Item = &Branch> {
        let mut branches = self.archive.management_all_branches().peekable();
        std::iter::from_fn(move || {
            loop {
                let mut preferred = branches.next()?;
                while let Some(next) =
                    branches.next_if(|next| next.conversation_id == preferred.conversation_id)
                {
                    if next.canonical && !preferred.canonical {
                        preferred = next;
                    }
                }
                if !self.management_conversation_terminal(&preferred.conversation_id) {
                    return Some(preferred);
                }
            }
        })
    }

    pub fn management_conversation_title(&self, conversation: &str) -> Option<&str> {
        self.archive.conversation_title(conversation)
    }

    pub fn management_branches<'a>(
        &'a self,
        conversation: &'a str,
    ) -> Option<impl Iterator<Item = &'a Branch>> {
        if self.archive.conversation_tombstone(conversation).is_some() {
            return None;
        }
        Some(self.archive.management_branches(conversation))
    }

    pub fn management_transcript_nodes(
        &self,
        conversation: &str,
        leaf: &str,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<String>), OwnerSessionError> {
        self.archive
            .management_transcript_nodes(conversation, leaf, offset, limit)
            .map_err(|error| match error {
                ArchiveError::MissingNode => OwnerSessionError::ManagementMissing,
                other => other.into(),
            })
    }

    pub fn management_conversation_terminal(&self, conversation: &str) -> bool {
        self.archive.conversation_tombstone(conversation).is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelSourceTurn {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content_id: [u8; 32],
    pub archive_revision: Option<u64>,
    pub content: Option<String>,
    pub terminal: bool,
    pub imported: Option<RelImportedSource>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RelImportedSource {
    pub namespace: String,
    pub format: String,
    pub source_digest: [u8; 32],
    pub receipt_digest: [u8; 32],
    pub conversation_id: String,
    pub message_id: String,
    pub message_revision: String,
    pub parent_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_author: Option<reliquary_archive::ImportedSourceAuthor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_branch_id: Option<String>,
}
