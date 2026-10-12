//! Sole operation boundary for explicit REL-local historical reconstruction.
//! The exclusively borrowed Cva owns freeze, planning, scoring and CVAFRS03 publication.
use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoricalReconstructionOptions {
    /// Request the bounded embedding-only operation before the final owner freeze.
    /// This is unavailable until its C7-C8 provider/resource contract is integrated.
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
}

#[derive(Debug)]
pub enum HistoricalReconstructionError {
    Owner(CvaError),
    EmbeddingFillUnavailable,
}

impl std::fmt::Display for HistoricalReconstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner(error) => write!(f, "historical reconstruction: {error}"),
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
            _ => None,
        }
    }
}
impl From<CvaError> for HistoricalReconstructionError {
    fn from(error: CvaError) -> Self {
        Self::Owner(error)
    }
}

impl Cva {
    /// Explicit offline/exclusive owner operation. Mounted callers must first
    /// use the current Gateway owner lifecycle/admission boundary.
    ///
    /// Accepts operation options only. Manifest, authorization, graph, vector,
    /// cohort and scoring evidence must be derived from this owner. Publication
    /// can only use this module's private freeze/prepare/commit primitives and
    /// the existing FreshnessStore CVAFRS03 checkpoint.
    ///
    /// Generation must be complete. Missing-embedding fill remains unavailable;
    /// accepted retries resolve before new owner-input checks and preserve live growth.
    pub fn reconstruct_historical_freshness(
        &mut self,
        options: HistoricalReconstructionOptions,
    ) -> Result<HistoricalReconstructionReport, HistoricalReconstructionError> {
        if options.fill_missing_embeddings {
            return Err(HistoricalReconstructionError::EmbeddingFillUnavailable);
        }
        let frozen = self.freeze_historical_import()?;
        let prepared = frozen.prepare_native()?;
        let committed = self.commit_historical_import(&prepared)?;
        Ok(HistoricalReconstructionReport {
            import_id: prepared.checkpoint.import_id,
            manifest_digest: prepared.checkpoint.manifest_digest,
            activity_cut: prepared.checkpoint.cut,
            reconstructed_memories: prepared.checkpoint.records.len(),
            committed,
        })
    }
}
