pub use reliquary_archive::{
    ImportReceipt, ImportedArtifact, ImportedBranch, ImportedConversation, ImportedMessage,
};
pub use reliquary_import::{
    ImportArtifact, ImportBranch, ImportConversation, ImportMessage, NativeImport,
};
use std::fmt;

pub use reliquary_import_store::{
    ImportComplete, ImportEpochStatus, ImportStart, ImportWatermarks,
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

#[derive(Debug)]
pub enum ImportError {
    Invalid(&'static str),
    Unsupported(&'static str),
    Json(serde_json::Error),
    Archive(crate::ArchiveError),
    Insomnia(crate::InsomniaError),
    Container(reliquary_container::ContainerError),
    Epoch(&'static str),
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
impl From<reliquary_container::ContainerError> for ImportError {
    fn from(e: reliquary_container::ContainerError) -> Self {
        Self::Container(e)
    }
}

impl From<reliquary_import_store::ImportError> for ImportError {
    fn from(error: reliquary_import_store::ImportError) -> Self {
        match error {
            reliquary_import_store::ImportError::Epoch(e) => Self::Epoch(e),
            reliquary_import_store::ImportError::Json(e) => Self::Json(e),
            reliquary_import_store::ImportError::Container(e) => Self::Container(e),
        }
    }
}

impl From<reliquary_import::ImportError> for ImportError {
    fn from(e: reliquary_import::ImportError) -> Self {
        match e {
            reliquary_import::ImportError::Invalid(e) => Self::Invalid(e),
            reliquary_import::ImportError::Unsupported(e) => Self::Unsupported(e),
            reliquary_import::ImportError::Json(e) => Self::Json(e),
            reliquary_import::ImportError::Archive(e) => Self::Archive(e),
        }
    }
}
