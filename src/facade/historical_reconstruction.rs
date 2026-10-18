//! Sole operation boundary for explicit REL-local historical reconstruction.
//! The exclusively borrowed Cva owns freeze, planning, scoring and settlement publication.
use super::*;

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
    Owner(CvaError),
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
    /// the FreshnessStore import-scoped settlement boundary.
    ///
    /// Generation must be complete. Fill requires an explicit bounded endpoint;
    /// accepted retries resolve before new owner-input checks and preserve live growth.
    pub fn reconstruct_historical_freshness(
        &mut self,
        options: HistoricalReconstructionOptions,
    ) -> Result<HistoricalReconstructionReport, HistoricalReconstructionError> {
        if self.open_import_epoch().is_some() {
            // Private freeze remains available for scoped evidence construction,
            // while the public publication boundary closes before prepare/no-op.
            return Err(HistoricalReconstructionError::Owner(fail(
                "epoch reconstruction requires authenticated physical-boundary settlement replay",
            )));
        }
        if options.fill_missing_embeddings && self.freshness.import_settlements().is_empty() {
            return Err(HistoricalReconstructionError::EmbeddingFillUnavailable);
        }
        let frozen = self.freeze_historical_import()?;
        // Keep scoped freeze construction available to the owner path, but do not
        // send it through the legacy prepare/commit representation.
        if frozen.epoch {
            return Err(HistoricalReconstructionError::Owner(fail(
                "epoch reconstruction requires authenticated physical-boundary settlement replay",
            )));
        }
        let prepared = frozen.prepare_native()?;
        let committed = self.commit_historical_import(&prepared)?;
        Ok(HistoricalReconstructionReport {
            import_id: prepared.settlement.import_id,
            manifest_digest: prepared.settlement.manifest_digest,
            activity_cut: prepared.settlement.settlement_cut,
            reconstructed_memories: prepared.settlement.records.len(),
            committed,
            embedding: None,
        })
    }
}

impl Cva {
    /// Offline/exclusive post-generation operation. Do not call with a mounted
    /// owner mutex held. Gateway mounted historical management is unsupported.
    /// Provider requests are serial, bounded and never retry internally.
    pub fn reconstruct_historical_freshness_with_endpoint(
        &mut self,
        options: HistoricalReconstructionOptions,
        endpoint: &impl crate::EmbeddingEndpoint,
        limits: HistoricalEmbeddingLimits,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<HistoricalReconstructionReport, HistoricalReconstructionError> {
        // A scoped epoch must not invoke an embedding provider before its
        // authenticated settlement replay contract exists.
        if self.open_import_epoch().is_some() {
            return Err(HistoricalReconstructionError::Owner(fail(
                "epoch reconstruction requires authenticated physical-boundary settlement replay",
            )));
        }
        if !self.freshness.import_settlements().is_empty() || !options.fill_missing_embeddings {
            return self.reconstruct_historical_freshness(options);
        }
        let report = embedding::fill(self, endpoint, limits, cancel)
            .map_err(|e| HistoricalReconstructionError::EmbeddingFill(Box::new(e)))?;
        if embedding::check_cancel(cancel).is_err() {
            return Err(embedding::cancelled_error(report));
        }
        let frozen = self
            .freeze_historical_import_with_fill(true)
            .map_err(|e| embedding::owner_error(e, report.clone()))?;
        let prepared = frozen
            .prepare_native()
            .map_err(|e| embedding::owner_error(e, report.clone()))?;
        if embedding::check_cancel(cancel).is_err() {
            return Err(embedding::cancelled_error(report));
        }
        // Once the native atomic append starts, complete acceptance/sync even if
        // cancellation races it. Never return cancelled for an accepted checkpoint.
        let committed = self
            .commit_historical_import(&prepared)
            .map_err(|e| embedding::owner_error(e, report.clone()))?;
        Ok(HistoricalReconstructionReport {
            import_id: prepared.settlement.import_id,
            manifest_digest: prepared.settlement.manifest_digest,
            activity_cut: prepared.settlement.settlement_cut,
            reconstructed_memories: prepared.settlement.records.len(),
            committed,
            embedding: Some(report),
        })
    }
}
