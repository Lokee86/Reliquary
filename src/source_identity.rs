//! Owner-qualified external source identity and source-span values.
//! This module contains no source integration, persistence, or codec behavior.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalMessageRef {
    pub source_namespace: String,
    pub conversation_id: String,
    pub message_id: String,
    pub content_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalChunkRef {
    pub message: ExternalMessageRef,
    /// UTF-8 byte offsets, validated against freshly hydrated canonical content.
    pub start: u64,
    pub end: u64,
}

/// Immutable accepted Project Environment file bindings; no mutation capability.
#[derive(Clone, Copy)]
pub(crate) struct ProjectFileFacts<'a> {
    records: &'a std::collections::HashMap<crate::FileId, crate::ProjectFileRef>,
}
impl<'a> ProjectFileFacts<'a> {
    pub(crate) fn new(
        records: &'a std::collections::HashMap<crate::FileId, crate::ProjectFileRef>,
    ) -> Self {
        Self { records }
    }

    pub(crate) fn get(&self, id: crate::FileId) -> Option<&crate::ProjectFileRef> {
        self.records.get(&id)
    }
    pub(crate) fn contains(&self, id: crate::FileId) -> bool {
        self.records.contains_key(&id)
    }
}
