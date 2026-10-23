//! Compatibility re-exports for the historical embedding public values.
pub use crate::imports::historical_embedding_model::{
    HistoricalEmbeddingCoverage, HistoricalEmbeddingDisposition, HistoricalEmbeddingFailure,
    HistoricalEmbeddingLimits, HistoricalEmbeddingReport,
};
pub(crate) use crate::imports::historical_embedding_model::{Stop, budget, unsupported};
