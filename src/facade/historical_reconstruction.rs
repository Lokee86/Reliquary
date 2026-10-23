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
        let open_epoch = if options.fill_missing_embeddings {
            self.open_import_epoch()
        } else {
            None
        };
        let accepted_settlement_exists = if options.fill_missing_embeddings && open_epoch.is_none()
        {
            Some(!self.freshness.import_settlements().is_empty())
        } else {
            None
        };
        crate::imports::historical_reconstruction::validate_initial_fill_admission(
            options.fill_missing_embeddings,
            open_epoch.as_ref(),
            accepted_settlement_exists,
        )?;
        let frozen = self.freeze_historical_import()?;
        let prepared = crate::imports::historical_reconstruction::prepare_without_endpoint(
            frozen,
            options.fill_missing_embeddings,
        )?;
        let committed = self.commit_historical_import(&prepared)?;
        Ok(crate::imports::historical_reconstruction::report(
            &prepared, committed, None,
        ))
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
        let accepted = match self.open_import_epoch() {
            Some(start) => self
                .freshness
                .import_settlements()
                .iter()
                .any(|settlement| {
                    settlement
                        .import_id
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                        == start.import_id
                }),
            None => !self.freshness.import_settlements().is_empty(),
        };
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
