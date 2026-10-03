use crate::archive_codec::{ArchiveRecord, decode_record, encode_content};
use crate::{Archive, ArchiveError, Container, ContentId, Node};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

impl Archive {
    pub(crate) fn empty() -> Self {
        Self {
            contents: Default::default(),
            nodes: Default::default(),
            activity: Default::default(),
            branches: Default::default(),
            conversations: Default::default(),
            fragments: Default::default(),
            episodes: Default::default(),
            files: Default::default(),
            source_attachments: Default::default(),
            file_memory_links: Default::default(),
            record_versions: Vec::new(),
            next_archive_version: 1,
        }
    }

    pub(crate) fn put_content(
        &mut self,
        container: &mut Container,
        id: ContentId,
        content: &str,
    ) -> Result<(), ArchiveError> {
        self.put_content_bytes(container, id, content.as_bytes())
    }

    pub(crate) fn put_content_bytes(
        &mut self,
        container: &mut Container,
        id: ContentId,
        content: &[u8],
    ) -> Result<(), ArchiveError> {
        if self.contents.contains(id) {
            if self.content_bytes(container, id)? != content {
                return Err(ArchiveError::HashCollision);
            }
            return Ok(());
        }
        let chunk = container.append(&encode_content(id, content)?)?;
        self.contents.insert(id, chunk);
        Ok(())
    }

    pub(crate) fn content(
        &self,
        container: &mut Container,
        id: ContentId,
    ) -> Result<String, ArchiveError> {
        String::from_utf8(self.content_bytes(container, id)?).map_err(|_| ArchiveError::InvalidUtf8)
    }

    pub(crate) fn content_bytes(
        &self,
        container: &mut Container,
        id: ContentId,
    ) -> Result<Vec<u8>, ArchiveError> {
        let chunk = self.contents.get(id).ok_or(ArchiveError::MissingContent)?;
        match decode_record(&container.read(chunk)?)? {
            ArchiveRecord::Content(found, bytes) if found == id => Ok(bytes),
            _ => Err(ArchiveError::MissingContent),
        }
    }

    pub(crate) fn branch_nodes(
        &self,
        conversation_id: &str,
        leaf_id: &str,
    ) -> Result<Vec<Node>, ArchiveError> {
        let mut nodes = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(leaf_id.to_owned());
        while let Some(id) = current {
            if !seen.insert(id.clone()) {
                return Err(ArchiveError::NodeCycle);
            }
            let node = self
                .require_node(conversation_id, &id, ArchiveError::MissingNode)?
                .clone();
            current = node.parent_id.clone();
            nodes.push(node);
        }
        nodes.reverse();
        Ok(nodes)
    }

    pub(crate) fn validate_references(
        &self,
        project_files: &crate::project_file_binding_store::ProjectFileStore,
    ) -> Result<(), ArchiveError> {
        for node in self.nodes.iter() {
            if !self.contents.contains(node.content_id) {
                return Err(ArchiveError::MissingContent);
            }
            self.validate_parent(&node.conversation_id, node.parent_id.as_deref())?;
        }
        for branch in self.branches.iter() {
            self.require_node(
                &branch.conversation_id,
                &branch.leaf_node_id,
                ArchiveError::MissingLeaf,
            )?;
            let mut seen = HashSet::new();
            let mut current = Some(branch.leaf_node_id.as_str());
            while let Some(id) = current {
                if !seen.insert(id) {
                    return Err(ArchiveError::NodeCycle);
                }
                let node =
                    self.require_node(&branch.conversation_id, id, ArchiveError::MissingNode)?;
                current = node.parent_id.as_deref();
            }
        }
        for metadata in self.conversations.iter() {
            if !self.has_conversation(&metadata.conversation_id) {
                return Err(ArchiveError::MissingConversation);
            }
        }
        self.validate_fragments()?;
        self.validate_episodes()?;
        self.validate_files(project_files)?;
        self.validate_file_memory_links()?;
        Ok(())
    }

    pub(crate) fn validate_parent(
        &self,
        conversation_id: &str,
        parent_id: Option<&str>,
    ) -> Result<(), ArchiveError> {
        if let Some(parent) = parent_id {
            self.require_node(conversation_id, parent, ArchiveError::MissingParent)?;
        }
        Ok(())
    }

    pub(crate) fn require_node<'a>(
        &'a self,
        conversation_id: &str,
        id: &str,
        error: ArchiveError,
    ) -> Result<&'a Node, ArchiveError> {
        self.nodes.get(conversation_id, id).ok_or(error)
    }
}

pub(crate) fn hash_content(bytes: &[u8]) -> ContentId {
    ContentId(Sha256::digest(bytes).into())
}

pub(crate) fn validate_text(value: &str, field: &'static str) -> Result<(), ArchiveError> {
    if value.is_empty() {
        Err(ArchiveError::InvalidField(field))
    } else {
        Ok(())
    }
}
