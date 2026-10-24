use crate::archive_codec::ArchiveRecord;
use crate::archive_object_index::{ContentIndex, FragmentIndex};
use crate::archive_record_index::{BranchIndex, NodeIndex};
use crate::archive_store::{hash_content, validate_text};
use crate::conversation_metadata_index::ConversationMetadataIndex;
use crate::file_index::FileIndex;
use crate::file_memory_link_index::FileMemoryLinkIndex;
use crate::source_attachment_index::SourceAttachmentIndex;
use crate::{
    ArchiveError, ArchiveRecordVersion, ArchiveStats, Branch, Container, ContentId, Fragment,
    FragmentId, Node, ResolvedTurn, StoredFile,
};

#[path = "archive_history_prune.rs"]
pub(crate) mod history_prune;

pub struct Archive {
    pub(crate) prune: history_prune::ArchivePruneState,
    pub(crate) import_receipts:
        std::collections::BTreeMap<String, crate::archive_import_model::ImportReceipt>,
    pub(crate) contents: ContentIndex,
    pub(crate) nodes: NodeIndex,
    pub(crate) activity: crate::archive_activity::ArchiveActivityIndex,
    pub(crate) branches: BranchIndex,
    pub(crate) conversations: ConversationMetadataIndex,
    pub(crate) conversation_metadata_versions: std::collections::BTreeMap<String, Vec<u64>>,
    pub(crate) conversation_tombstones:
        std::collections::BTreeMap<String, crate::ConversationTombstone>,
    pub(crate) fragments: FragmentIndex,
    pub(crate) episodes: crate::episode_index::EpisodeIndex,
    pub(crate) files: FileIndex,
    pub(crate) source_attachments: SourceAttachmentIndex,
    pub(crate) file_memory_links: FileMemoryLinkIndex,
    pub(crate) record_versions: Vec<ArchiveRecordVersion>,
    pub(crate) next_archive_version: u64,
}

/// Container-neutral read contract for Archive-owned source facts and text.
/// It binds the existing owner handles without opening a new snapshot or lease.
pub(crate) struct ArchiveReadView<'a> {
    archive: &'a Archive,
    container: &'a Container,
}

impl<'a> ArchiveReadView<'a> {
    pub(crate) fn new(archive: &'a Archive, container: &'a Container) -> Self {
        Self { archive, container }
    }

    pub(crate) fn fragments_from(&self, start: usize) -> Vec<Fragment> {
        self.archive.fragments.iter().skip(start).cloned().collect()
    }

    pub(crate) fn fragment(&self, id: FragmentId) -> Option<Fragment> {
        self.archive.fragments.get(id).cloned()
    }

    pub(crate) fn fragment_archive_version(&self, id: FragmentId) -> Option<u64> {
        self.archive.fragments.archive_version(id)
    }

    pub(crate) fn files_from(&self, start: usize) -> Vec<StoredFile> {
        self.archive.files.iter().skip(start).cloned().collect()
    }

    pub(crate) fn nodes_from(&self, start: usize) -> Vec<Node> {
        self.archive.nodes.iter().skip(start).cloned().collect()
    }

    pub(crate) fn branch_nodes(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<Node>, ArchiveError> {
        self.archive.branch_nodes(conversation_id, leaf_node_id)
    }

    pub(crate) fn require_current_conversation_id(
        &self,
        conversation_id: &str,
    ) -> Result<(), ArchiveError> {
        self.archive
            .require_current_conversation_id(conversation_id)
    }

    pub(crate) fn fragment_nodes_for(
        &self,
        fragment: &Fragment,
    ) -> Result<Vec<Node>, ArchiveError> {
        self.archive.fragment_nodes_for(fragment)
    }

    pub(crate) fn content(&self, id: ContentId) -> Result<String, ArchiveError> {
        self.archive.content(self.container, id)
    }

    pub(crate) fn fragment_text(&self, id: FragmentId) -> Result<String, ArchiveError> {
        let fragment = self.fragment(id).ok_or(ArchiveError::MissingFragment)?;
        self.require_current_conversation_id(&fragment.conversation_id)?;
        self.fragment_text_for(&fragment)
    }

    pub(crate) fn fragment_text_for(&self, fragment: &Fragment) -> Result<String, ArchiveError> {
        self.fragment_nodes_for(fragment)?
            .into_iter()
            .map(|node| Ok(format!("{}: {}", node.role, self.content(node.content_id)?)))
            .collect::<Result<Vec<_>, ArchiveError>>()
            .map(|turns| turns.join("\n"))
    }
}

impl Archive {
    pub(crate) fn append_node(
        &mut self,
        container: &mut Container,
        id: String,
        conversation_id: String,
        parent_id: Option<String>,
        role: String,
        principal_id: Option<String>,
        timestamp_ns: i64,
        content: &str,
    ) -> Result<crate::archive_activity::TurnAcceptance<Node>, ArchiveError> {
        validate_text(&id, "node id")?;
        validate_text(&conversation_id, "conversation id")?;
        self.require_current_conversation_id(&conversation_id)?;
        validate_text(&role, "role")?;
        if let Some(principal_id) = principal_id.as_deref() {
            validate_text(principal_id, "principal id")?;
        }
        self.validate_parent(&conversation_id, parent_id.as_deref())?;

        let content_id = hash_content(content.as_bytes());
        let node = Node {
            id,
            conversation_id,
            parent_id,
            role,
            principal_id,
            timestamp_ns,
            content_id,
        };
        let Some(prepared) = self.activity.prepare_node(&mut self.nodes, &node)? else {
            let ordinal = self.nodes.ordinal(&node.conversation_id, &node.id).unwrap();
            return Ok(self.activity.acceptance(node, ordinal, false));
        };
        let ordinal = self.nodes.len();
        let prepared_publication =
            self.prepare_publication(container, ArchiveRecord::Node(node.clone()))?;
        self.put_content(container, content_id, content)?;
        let version = self.publish_prepared(container, prepared_publication)?;
        self.nodes.insert(node.clone())?;
        self.activity.commit_new(prepared, version.archive_version);
        Ok(self.activity.acceptance(node, ordinal, true))
    }

    pub(crate) fn append_branch(
        &mut self,
        container: &mut Container,
        branch: Branch,
    ) -> Result<(), ArchiveError> {
        validate_text(&branch.id, "branch id")?;
        validate_text(&branch.conversation_id, "conversation id")?;
        self.require_current_conversation_id(&branch.conversation_id)?;
        self.require_node(
            &branch.conversation_id,
            &branch.leaf_node_id,
            ArchiveError::MissingLeaf,
        )?;
        if self.branches.get(&branch.conversation_id, &branch.id) == Some(&branch) {
            return Ok(());
        }
        if let Some(existing) = self.branches.get(&branch.conversation_id, &branch.id) {
            let path = self.branch_nodes(&branch.conversation_id, &branch.leaf_node_id)?;
            if !path.iter().any(|node| node.id == existing.leaf_node_id) {
                return Err(ArchiveError::InvalidBranchRevision);
            }
        }
        self.publish_publication(container, ArchiveRecord::Branch(branch.clone()))?;
        self.branches.put(branch);
        Ok(())
    }

    pub(crate) fn branch_turns(
        &self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.require_current_conversation_id(conversation_id)?;
        self.historical_branch_turns(container, conversation_id, branch_id)
    }

    pub(crate) fn historical_branch_turns(
        &self,
        container: &mut Container,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        let branch = self
            .branches
            .get(conversation_id, branch_id)
            .ok_or(ArchiveError::MissingBranch)?
            .clone();
        let nodes = self.branch_nodes(conversation_id, &branch.leaf_node_id)?;
        nodes
            .into_iter()
            .map(|node| {
                let content = self.node_content(container, &node)?;
                Ok(ResolvedTurn {
                    node_id: node.id,
                    role: node.role,
                    principal_id: node.principal_id,
                    timestamp_ns: node.timestamp_ns,
                    content,
                })
            })
            .collect()
    }

    pub fn branches(&self) -> Vec<Branch> {
        self.branches
            .iter()
            .filter(|branch| !self.is_conversation_tombstoned(&branch.conversation_id))
            .cloned()
            .collect()
    }

    pub fn historical_branches(&self) -> Vec<Branch> {
        self.branches.iter().cloned().collect()
    }

    pub(crate) fn has_node(&self, conversation_id: &str, node_id: &str) -> bool {
        self.nodes.get(conversation_id, node_id).is_some()
    }

    pub(crate) fn has_conversation(&self, conversation_id: &str) -> bool {
        !self.is_conversation_tombstoned(conversation_id)
            && self.has_historical_conversation(conversation_id)
    }

    pub(crate) fn has_historical_conversation(&self, conversation_id: &str) -> bool {
        self.nodes
            .iter()
            .any(|node| node.conversation_id == conversation_id)
    }

    pub(crate) fn require_current_conversation_id(
        &self,
        conversation_id: &str,
    ) -> Result<(), ArchiveError> {
        if self.is_conversation_tombstoned(conversation_id) {
            Err(ArchiveError::TombstonedConversation)
        } else {
            Ok(())
        }
    }

    pub fn stats(&self) -> ArchiveStats {
        ArchiveStats {
            content_objects: self.contents.len(),
            nodes: self.nodes.len(),
            branches: self.branches.len(),
            fragments: self.fragments.len(),
            episodes: self.episodes.len(),
            files: self.files.len(),
            source_attachments: self.source_attachments.len(),
            file_memory_links: self.file_memory_links.len(),
        }
    }
}
