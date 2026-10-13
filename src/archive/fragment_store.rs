use crate::archive_codec::encode_fragment;
use crate::{Archive, ArchiveError, Container, Fragment, FragmentId, Node, ResolvedTurn};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

impl Archive {
    pub(crate) fn put_fragment(
        &mut self,
        container: &mut Container,
        fragment: Fragment,
    ) -> Result<bool, ArchiveError> {
        self.validate_fragment(&fragment)?;
        if let Some(existing) = self.fragments.get(fragment.id) {
            return if existing == &fragment {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingFragment)
            };
        }
        let record = container.append(&encode_fragment(&fragment)?)?;
        let version = self.publish_record(container, record)?;
        self.fragments.insert(fragment, version.archive_version)?;
        Ok(true)
    }

    pub(crate) fn fragment_nodes_for(
        &self,
        fragment: &Fragment,
    ) -> Result<Vec<Node>, ArchiveError> {
        let mut nodes = Vec::new();
        let mut seen = HashSet::new();
        let mut current = fragment.end_node_id.clone();
        loop {
            if !seen.insert(current.clone()) {
                return Err(ArchiveError::NodeCycle);
            }
            let node = self
                .nodes
                .get(&fragment.conversation_id, &current)
                .ok_or(ArchiveError::MissingNode)?
                .clone();
            let done = node.id == fragment.start_node_id;
            current = node.parent_id.clone().unwrap_or_default();
            nodes.push(node);
            if done {
                break;
            }
            if current.is_empty() {
                return Err(ArchiveError::InvalidFragmentRange);
            }
        }
        nodes.reverse();
        Ok(nodes)
    }

    pub(crate) fn validate_fragments(&self) -> Result<(), ArchiveError> {
        for fragment in self.fragments.iter() {
            self.validate_fragment(fragment)?;
        }
        Ok(())
    }

    pub(crate) fn validate_fragment(&self, fragment: &Fragment) -> Result<(), ArchiveError> {
        if fragment.id
            != fragment_id(
                &fragment.conversation_id,
                &fragment.start_node_id,
                &fragment.end_node_id,
            )
        {
            return Err(ArchiveError::InvalidFragmentId);
        }
        let mut seen = HashSet::new();
        let mut current = Some(fragment.end_node_id.as_str());
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(ArchiveError::NodeCycle);
            }
            let node = self
                .nodes
                .get(&fragment.conversation_id, id)
                .ok_or(ArchiveError::MissingNode)?;
            if id == fragment.start_node_id.as_str() {
                return Ok(());
            }
            current = node.parent_id.as_deref();
        }
        Err(ArchiveError::InvalidFragmentRange)
    }

    pub(crate) fn fragment_turns(
        &self,
        container: &mut Container,
        id: FragmentId,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        let fragment = self
            .fragments
            .get(id)
            .ok_or(ArchiveError::MissingFragment)?
            .clone();
        let nodes = self.fragment_nodes_for(&fragment)?;
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

    pub(crate) fn fragment_text(
        &self,
        container: &mut Container,
        id: FragmentId,
    ) -> Result<String, ArchiveError> {
        let fragment = self
            .fragments
            .get(id)
            .ok_or(ArchiveError::MissingFragment)?
            .clone();
        self.fragment_text_for(container, &fragment)
    }

    pub(crate) fn fragment_text_for(
        &self,
        container: &mut Container,
        fragment: &Fragment,
    ) -> Result<String, ArchiveError> {
        let nodes = self.fragment_nodes_for(fragment)?;
        nodes
            .into_iter()
            .map(|node| {
                Ok(format!(
                    "{}: {}",
                    node.role,
                    self.content(container, node.content_id)?
                ))
            })
            .collect::<Result<Vec<_>, ArchiveError>>()
            .map(|turns| turns.join("\n"))
    }

    /// Ordinary genuine-fragment rendering with a total physical hydration guard.
    pub(crate) fn fragment_text_bounded(
        &self,
        container: &mut Container,
        fragment: &Fragment,
        max_bytes: usize,
    ) -> Result<String, ArchiveError> {
        self.validate_fragment(fragment)?;
        let mut text = String::new();
        for node in self.fragment_nodes_for(fragment)? {
            let separator = usize::from(!text.is_empty());
            let overhead = node
                .role
                .len()
                .checked_add(2)
                .and_then(|n| n.checked_add(separator))
                .ok_or(ArchiveError::FieldTooLarge)?;
            let remaining = max_bytes
                .checked_sub(text.len())
                .and_then(|n| n.checked_sub(overhead))
                .ok_or(ArchiveError::FieldTooLarge)?;
            let cap = remaining
                .checked_add(44)
                .ok_or(ArchiveError::FieldTooLarge)?;
            let object = self
                .contents
                .get(node.content_id)
                .ok_or(ArchiveError::MissingContent)?;
            let payload = container.read_bounded(object, cap)?;
            let crate::archive_codec::ArchiveRecord::Content(id, bytes) =
                crate::archive_codec::decode_record(&payload)?
            else {
                return Err(ArchiveError::MissingContent);
            };
            if id != node.content_id || crate::archive_store::hash_content(&bytes) != id {
                return Err(ArchiveError::HashCollision);
            }
            let content = String::from_utf8(bytes).map_err(|_| ArchiveError::InvalidUtf8)?;
            if content.len() > remaining {
                return Err(ArchiveError::FieldTooLarge);
            }
            if separator != 0 {
                text.push('\n');
            }
            text.push_str(&node.role);
            text.push_str(": ");
            text.push_str(&content);
        }
        Ok(text)
    }

    pub fn fragments(&self) -> Vec<Fragment> {
        let mut fragments: Vec<_> = self.fragments.iter().cloned().collect();
        fragments.sort_by_key(|fragment| fragment.id.0);
        fragments
    }
}

pub(crate) fn fragment_id(
    conversation_id: &str,
    start_node_id: &str,
    end_node_id: &str,
) -> FragmentId {
    let mut hash = Sha256::new();
    for value in [conversation_id, start_node_id, end_node_id] {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    FragmentId(hash.finalize().into())
}
