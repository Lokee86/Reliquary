//! Sole operation boundary for explicit REL-local historical reconstruction.
//! The exclusively borrowed Cva owns freeze, planning, scoring and settlement publication.
use super::*;
pub use crate::imports::historical_reconstruction_model::{
    HistoricalReconstructionError, HistoricalReconstructionOptions, HistoricalReconstructionReport,
};

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
        self.runtime_reconstruct_historical_freshness(options)
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
        if !options.fill_missing_embeddings {
            return self.reconstruct_historical_freshness(options);
        }
        let open_epoch = self.open_import_epoch();
        let accepted = self.runtime_has_accepted_historical_settlement(
            open_epoch.as_ref().map(|start| start.import_id.as_str()),
        );
        if accepted {
            return self.reconstruct_historical_freshness(options);
        }
        // This is the explicit exclusive historical owner operation. Scope only
        // its bounded publication work; ordinary live mutation remains fenced.
        let report = if self.open_import_epoch().is_some() {
            embedding::fill_open_epoch(self, endpoint, limits, cancel)
        } else {
            embedding::fill(self, endpoint, limits, cancel)
        }
        .map_err(|e| HistoricalReconstructionError::EmbeddingFill(Box::new(e)))?;
        if embedding::check_cancel(cancel).is_err() {
            return Err(embedding::cancelled_error(report));
        }
        let frozen = self
            .freeze_historical_import_with_fill(true)
            .map_err(|e| embedding::owner_error(e, report.clone()))?;
        let prepared =
            crate::imports::historical_reconstruction::prepare_frozen(frozen).map_err(|error| {
                match error {
                    HistoricalReconstructionError::Owner(error) => {
                        embedding::owner_error(error, report.clone())
                    }
                    error => error,
                }
            })?;
        if embedding::check_cancel(cancel).is_err() {
            return Err(embedding::cancelled_error(report));
        }
        // Once the native atomic append starts, complete acceptance/sync even if
        // cancellation races it. Never return cancelled for an accepted checkpoint.
        let committed = self
            .commit_historical_import(&prepared)
            .map_err(|e| embedding::owner_error(e, report.clone()))?;
        Ok(crate::imports::historical_reconstruction::report(
            &prepared,
            committed,
            Some(report),
        ))
    }
}
