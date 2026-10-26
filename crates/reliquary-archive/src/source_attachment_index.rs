use crate::{ArchiveError, FileId};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct SourceAttachmentIndex {
    by_conversation: HashMap<String, HashMap<String, Vec<FileId>>>,
    links: usize,
}

impl SourceAttachmentIndex {
    pub(crate) fn len(&self) -> usize {
        self.links
    }

    pub(crate) fn insert(
        &mut self,
        conversation_id: &str,
        node_id: &str,
        file_ids: Vec<FileId>,
    ) -> Result<bool, ArchiveError> {
        let nodes = self
            .by_conversation
            .entry(conversation_id.to_owned())
            .or_default();
        if let Some(existing) = nodes.get(node_id) {
            return if existing == &file_ids {
                Ok(false)
            } else {
                Err(ArchiveError::ConflictingTurnIngest)
            };
        }
        self.links += file_ids.len();
        nodes.insert(node_id.to_owned(), file_ids);
        Ok(true)
    }

    pub(crate) fn get(&self, conversation_id: &str, node_id: &str) -> Option<&[FileId]> {
        self.by_conversation
            .get(conversation_id)?
            .get(node_id)
            .map(Vec::as_slice)
    }

    pub(crate) fn conversation_file_ids(&self, conversation_id: &str) -> Vec<FileId> {
        self.by_conversation
            .get(conversation_id)
            .into_iter()
            .flat_map(|nodes| nodes.values())
            .flatten()
            .copied()
            .collect()
    }
}
