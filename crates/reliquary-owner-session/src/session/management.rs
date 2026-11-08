use super::*;
use reliquary_memory_core::{MemoryDraft, MemoryRevision};
use std::collections::BTreeMap;

#[path = "management_inspection.rs"]
mod inspection;
pub use inspection::*;
#[path = "management_archive.rs"]
mod archive_inspection;
pub use archive_inspection::{RelImportedSource, RelSourceTurn};
#[path = "management_community.rs"]
mod community;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelConversationSummary {
    pub conversation_id: String,
    pub branch_id: String,
    pub leaf_node_id: String,
    pub canonical: bool,
    pub title: Option<String>,
    pub turn_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelTranscriptTurn {
    pub node_id: String,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
}

impl RelSession {
    /// Return one deterministic preferred branch per visible conversation.
    pub fn conversation_summaries(&self) -> Result<Vec<RelConversationSummary>, OwnerSessionError> {
        let mut selected = BTreeMap::<String, reliquary_archive::Branch>::new();
        for branch in self.archive.branches() {
            let replace = selected.get(&branch.conversation_id).is_none_or(|current| {
                (branch.canonical && !current.canonical)
                    || (branch.canonical == current.canonical && branch.id < current.id)
            });
            if replace {
                selected.insert(branch.conversation_id.clone(), branch);
            }
        }
        selected
            .into_values()
            .map(|branch| {
                let turns = self
                    .archive
                    .historical_turns(&branch.conversation_id, &branch.leaf_node_id)?;
                Ok(RelConversationSummary {
                    conversation_id: branch.conversation_id.clone(),
                    branch_id: branch.id,
                    leaf_node_id: branch.leaf_node_id,
                    canonical: branch.canonical,
                    title: self
                        .archive
                        .conversation_title(&branch.conversation_id)
                        .map(str::to_owned),
                    turn_count: turns.len(),
                })
            })
            .collect()
    }

    pub fn transcript(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<RelTranscriptTurn>, OwnerSessionError> {
        Ok(self
            .archive
            .historical_turns(conversation_id, leaf_node_id)?
            .into_iter()
            .map(|turn| RelTranscriptTurn {
                node_id: turn.node_id,
                role: turn.role,
                principal_id: turn.principal_id,
                timestamp_ns: turn.timestamp_ns,
                content: turn.content,
            })
            .collect())
    }
}

impl RelWriter {
    pub fn set_conversation_title(
        &mut self,
        conversation_id: &str,
        title: &str,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let result = self.session.archive.set_conversation_title(
            &mut self.container,
            conversation_id,
            title,
        );
        match result {
            Ok(changed) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(changed)
            }
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }

    /// Publish one application-validated direct owner Memory.
    pub fn publish_direct_memory(
        &mut self,
        principal: &str,
        draft: MemoryDraft,
    ) -> Result<MemoryRevision, OwnerSessionError> {
        self.ensure_write_ready()?;
        let expected_owner = self
            .owner_id()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let source = draft.source_ref.as_ref().ok_or_else(|| {
            OwnerSessionError::Record("direct Memory requires durable provenance".into())
        })?;
        if principal.trim().is_empty()
            || source.owner_id != expected_owner
            || source.principal_id.as_deref() != Some(principal)
            || draft.authority_kind != "direct"
            || draft.source_episode_id.is_some()
        {
            return Err(OwnerSessionError::Record(
                "direct Memory provenance does not match its owner principal".into(),
            ));
        }
        let before = self.container.writer_lease_proof();
        let published = self
            .session
            .common
            .memories
            .publish(&mut self.container, None, 0, draft);
        match published {
            Ok(result) => {
                if let Err(error) = self.container.sync() {
                    self.session.write_poisoned = true;
                    return Err(error.into());
                }
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                if result.published {
                    if let Err(error) = self
                        .session
                        .common
                        .refresh_dream_index(&[result.revision.record.id])
                    {
                        self.session.write_poisoned = true;
                        return Err(error);
                    }
                }
                Ok(result.revision)
            }
            Err(error) => {
                if before != self.container.writer_lease_proof() {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}
