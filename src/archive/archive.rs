use crate::archive_codec::{encode_branch, encode_node};
use crate::archive_object_index::{ContentIndex, FragmentIndex};
use crate::archive_record_index::{BranchIndex, NodeIndex};
use crate::archive_store::{hash_content, validate_text};
use crate::conversation_metadata_index::ConversationMetadataIndex;
use crate::file_index::FileIndex;
use crate::file_memory_link_index::FileMemoryLinkIndex;
use crate::source_attachment_index::SourceAttachmentIndex;
use crate::{
    ArchiveError, ArchiveRecordVersion, ArchiveStats, Branch, Container, Node, ResolvedTurn,
};

pub struct Archive {
    pub(crate) contents: ContentIndex,
    pub(crate) nodes: NodeIndex,
    pub(crate) activity: crate::archive_activity::ArchiveActivityIndex,
    pub(crate) branches: BranchIndex,
    pub(crate) conversations: ConversationMetadataIndex,
    pub(crate) fragments: FragmentIndex,
    pub(crate) episodes: crate::episode_index::EpisodeIndex,
    pub(crate) files: FileIndex,
    pub(crate) source_attachments: SourceAttachmentIndex,
    pub(crate) file_memory_links: FileMemoryLinkIndex,
    pub(crate) record_versions: Vec<ArchiveRecordVersion>,
    pub(crate) next_archive_version: u64,
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
        let payload = encode_node(&node)?;
        self.put_content(container, content_id, content)?;
        let record = container.append(&payload)?;
        let version = self.publish_record(container, record)?;
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
        let record = container.append(&encode_branch(&branch)?)?;
        self.publish_record(container, record)?;
        self.branches.put(branch);
        Ok(())
    }

    pub(crate) fn branch_turns(
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
                let content = self.content(container, node.content_id)?;
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
        self.branches.iter().cloned().collect()
    }

    pub(crate) fn has_node(&self, conversation_id: &str, node_id: &str) -> bool {
        self.nodes.get(conversation_id, node_id).is_some()
    }

    pub(crate) fn has_conversation(&self, conversation_id: &str) -> bool {
        self.nodes
            .iter()
            .any(|node| node.conversation_id == conversation_id)
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
