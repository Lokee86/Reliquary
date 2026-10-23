//! Stable public result and error values for historical reconstruction.
use super::historical_embedding_model::{HistoricalEmbeddingFailure, HistoricalEmbeddingReport};
use crate::owner_lifecycle::OwnerError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoricalReconstructionOptions {
    /// Request the bounded embedding-only operation before the final owner freeze.
    /// Use the explicit endpoint or ConfiguredRuntime post-generation operation.
    pub fill_missing_embeddings: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalReconstructionReport {
    pub import_id: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub activity_cut: u64,
    pub reconstructed_memories: usize,
    /// False only for an exact accepted retry; true after native append and sync.
    pub committed: bool,
    /// Transient fill cost/coverage, absent for no-fill and accepted retries.
    pub embedding: Option<HistoricalEmbeddingReport>,
}

#[derive(Debug)]
pub enum HistoricalReconstructionError {
    Owner(OwnerError),
    EmbeddingFillUnavailable,
    EmbeddingFill(Box<HistoricalEmbeddingFailure>),
}

impl std::fmt::Display for HistoricalReconstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner(error) => write!(f, "historical reconstruction: {error}"),
            Self::EmbeddingFill(error) => error.fmt(f),
            Self::EmbeddingFillUnavailable => {
                f.write_str("historical embedding-only fill is unavailable")
            }
        }
    }
}
impl std::error::Error for HistoricalReconstructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::EmbeddingFill(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
impl From<OwnerError> for HistoricalReconstructionError {
    fn from(error: OwnerError) -> Self {
        Self::Owner(error)
    }
}
