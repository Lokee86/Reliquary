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
pub use crate::archive_import_model::{
    ImportReceipt, ImportedArtifact, ImportedBranch, ImportedConversation, ImportedMessage,
};

pub use super::epoch_model::{
    ImportComplete, ImportEpochStatus, ImportError, ImportStart, ImportWatermarks,
};

/// Import owner whose exact evidence prevents completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportReadinessOwner {
    Archive,
    Insomnia,
    Memory,
    Dream,
    Semantic,
    Perception,
    Ego,
    Freshness,
}

/// Why one import-attributed item is not ready to complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportReadinessStatus {
    Missing,
    Pending,
    Failed,
    ConflictingEvidence,
    Unsupported,
}

/// A bounded blocker tied to one import-owned identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportReadinessBlocker {
    pub owner: ImportReadinessOwner,
    pub identity: String,
    pub status: ImportReadinessStatus,
}

/// Vector support is deliberately not inferred from unrelated owner counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportVectorReadiness {
    /// No imported source Nodes or local Insomnia Memory outputs require coverage.
    NotRequired,
    /// Native REL imports do not currently certify exact vector coverage.
    SupportedUnavailable,
}

/// Owner-derived, read-only readiness evidence for the currently open import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportReadinessReport {
    pub import_id: String,
    pub blocker: Option<ImportReadinessBlocker>,
    pub vectors: ImportVectorReadiness,
}

/// An inspectable reason an exclusive completion attempt left its epoch open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportCompletionPending {
    MissingReceipt,
    MissingFreshnessSettlement,
    Readiness(ImportReadinessBlocker),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportCompletionOutcome {
    Pending(ImportCompletionPending),
    Completed {
        complete: ImportComplete,
        /// False when an already accepted marker was synchronized and reused.
        committed: bool,
    },
}
