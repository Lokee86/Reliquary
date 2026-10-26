//! Immutable source facts for higher owner composition.
use crate::{
    Archive, ContentId, ConversationMetadata, FileMemoryLink, Fragment, FragmentId, ImportReceipt,
    Node,
};
use std::collections::BTreeMap;

impl Archive {
    /// Iterate stored source Nodes without exposing the mutable index.
    pub fn source_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// Look up one source Node by its conversation-local identity.
    pub fn source_node(&self, conversation_id: &str, node_id: &str) -> Option<&Node> {
        self.nodes.get(conversation_id, node_id)
    }

    pub fn source_node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Iterate immutable source fragments without exposing the mutable index.
    pub fn source_fragments(&self) -> impl Iterator<Item = &Fragment> {
        self.fragments.iter()
    }

    pub fn source_fragment(&self, id: FragmentId) -> Option<&Fragment> {
        self.fragments.get(id)
    }

    pub fn source_fragment_archive_version(&self, id: FragmentId) -> Option<u64> {
        self.fragments.archive_version(id)
    }

    pub fn source_fragment_count(&self) -> usize {
        self.fragments.len()
    }

    pub fn all_file_memory_links(&self) -> impl Iterator<Item = &FileMemoryLink> {
        self.file_memory_links.iter()
    }

    pub fn has_content(&self, content_id: ContentId) -> bool {
        self.contents.contains(content_id)
    }
}

impl Archive {
    /// Returns persisted conversation metadata in source order, including
    /// metadata for conversations that have since been tombstoned.
    pub fn historical_conversation_metadata_records(&self) -> Vec<ConversationMetadata> {
        self.conversations.iter().cloned().collect()
    }

    /// Iterate immutable source files for derived consumer indexes.
    pub fn source_files(&self) -> impl Iterator<Item = &crate::StoredFile> {
        self.files.iter()
    }
}

impl Archive {
    /// Return one persisted import receipt by its stable import key.
    pub fn import_receipt(&self, import_key: &str) -> Option<&ImportReceipt> {
        self.import_receipts.get(import_key)
    }

    /// Return persisted import receipts keyed by their stable import keys.
    pub fn import_receipts(&self) -> &BTreeMap<String, ImportReceipt> {
        &self.import_receipts
    }

    /// Return all persisted import receipts in deterministic key order.
    pub fn all_import_receipts(&self) -> Vec<ImportReceipt> {
        self.import_receipts.values().cloned().collect()
    }

    /// Check whether Archive currently retains a branch with this identity.
    pub fn has_branch(&self, conversation_id: &str, branch_id: &str) -> bool {
        self.branches.get(conversation_id, branch_id).is_some()
    }
}
