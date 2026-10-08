use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema 1 imports complete, explicitly supplied snapshots. Omission is not deletion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeImport {
    pub schema_version: u32,
    /// Stable provider/account namespace supplied by the caller, never a filename.
    pub source_namespace: String,
    pub source_format: String,
    pub conversations: Vec<ImportConversation>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportConversation {
    pub id: String,
    pub title: Option<String>,
    pub messages: Vec<ImportMessage>,
    pub branches: Vec<ImportBranch>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMessage {
    pub id: String,
    /// Source revision if exported; otherwise None derives a content revision.
    pub revision: Option<String>,
    pub parent_id: Option<String>,
    pub replaces: Option<String>,
    pub deleted: bool,
    pub role: String,
    pub timestamp_ns: i64,
    pub content: String,
    pub artifacts: Vec<ImportArtifact>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportBranch {
    pub id: String,
    pub leaf_message_id: String,
    pub canonical: bool,
}
/// Artifact bytes must be supplied explicitly. Import never follows a URL or filesystem path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportArtifact {
    pub id: String,
    pub source_ref: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}
/// Archive-owned source evidence, with references/hashes rather than another transcript copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReceipt {
    pub schema_version: u32,
    pub import_key: String,
    pub source_namespace: String,
    pub source_format: String,
    pub conversations: Vec<ImportedConversation>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedConversation {
    pub source_id: String,
    pub conversation_id: String,
    pub title: Option<String>,
    pub messages: Vec<ImportedMessage>,
    pub branches: Vec<ImportedBranch>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedMessage {
    pub source_id: String,
    pub revision: String,
    pub source_parent_id: Option<String>,
    pub replaces: Option<String>,
    pub deleted: bool,
    pub node_id: Option<String>,
    pub fingerprint: String,
    pub artifacts: Vec<ImportedArtifact>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedArtifact {
    pub source_id: String,
    pub source_ref: String,
    pub sha256: String,
    pub file_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedBranch {
    pub source_id: String,
    pub branch_id: String,
    /// Empty/deleted branch has no native head; its source evidence is still retained.
    pub leaf_node_id: Option<String>,
    pub canonical: bool,
}
#[derive(Debug)]
pub enum ImportError {
    Invalid(&'static str),
    Unsupported(&'static str),
    Json(serde_json::Error),
    Archive(crate::ArchiveError),
    Insomnia(crate::InsomniaError),
    Container(crate::ContainerError),
}
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native import error: {self:?}")
    }
}
impl std::error::Error for ImportError {}
impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<crate::ArchiveError> for ImportError {
    fn from(e: crate::ArchiveError) -> Self {
        Self::Archive(e)
    }
}
impl From<crate::InsomniaError> for ImportError {
    fn from(e: crate::InsomniaError) -> Self {
        Self::Insomnia(e)
    }
}
impl From<crate::ContainerError> for ImportError {
    fn from(e: crate::ContainerError) -> Self {
        Self::Container(e)
    }
}
