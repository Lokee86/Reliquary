//! Explicit offline post-generation management; no background activation.
use super::ConfiguredRuntime;
use crate::{
    Cva, HistoricalEmbeddingFailure, HistoricalEmbeddingLimits, HistoricalEmbeddingReport,
    HistoricalReconstructionError, HistoricalReconstructionOptions, HistoricalReconstructionReport,
};
use std::sync::atomic::AtomicBool;
impl ConfiguredRuntime {
    /// Operate on an exclusively held offline REL after import/Insomnia/Dream
    /// generation completes. Mounted Gateway callers must not use this method.
    pub fn reconstruct_historical_freshness(
        &self,
        rel: &mut Cva,
        options: HistoricalReconstructionOptions,
        limits: HistoricalEmbeddingLimits,
        cancel: &AtomicBool,
    ) -> Result<HistoricalReconstructionReport, HistoricalReconstructionError> {
        if !options.fill_missing_embeddings || rel.freshness.import_checkpoint().is_some() {
            return rel.reconstruct_historical_freshness(options);
        }
        let endpoint = self.embedding_endpoint(16, 1).map_err(|e| {
            HistoricalReconstructionError::EmbeddingFill(Box::new(HistoricalEmbeddingFailure {
                report: HistoricalEmbeddingReport::default(),
                message: e.to_string(),
            }))
        })?;
        rel.reconstruct_historical_freshness_with_endpoint(options, &endpoint, limits, cancel)
    }
}
