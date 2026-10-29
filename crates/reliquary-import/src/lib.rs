//! Deterministic native import normalization and preparation.
use serde::{Deserialize, Serialize};
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

pub use reliquary_archive::{
    ImportReceipt, ImportedArtifact, ImportedBranch, ImportedConversation, ImportedMessage,
};

#[derive(Debug)]
pub enum ImportError {
    Invalid(&'static str),
    Unsupported(&'static str),
    Json(serde_json::Error),
    Archive(reliquary_archive::ArchiveError),
}
impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<reliquary_archive::ArchiveError> for ImportError {
    fn from(e: reliquary_archive::ArchiveError) -> Self {
        Self::Archive(e)
    }
}
mod prepare;
pub use prepare::{PreparedImport, digest, prepare};
mod providers;
pub use providers::{normalize_agent, normalize_chatgpt, normalize_claude, normalize_hermes_jsonl};
