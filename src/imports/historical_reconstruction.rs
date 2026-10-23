//! Import-owned public reconstruction policy over a frozen owner-derived plan.
use super::historical_settlement_plan::{FrozenHistoricalImport, PreparedHistoricalImport};

use super::historical_embedding_model::HistoricalEmbeddingReport;
use super::historical_reconstruction_model::{
    HistoricalReconstructionError, HistoricalReconstructionReport,
};

/// Validate whether no-endpoint fill was requested with an existing import context.
/// `accepted_settlement_exists` is read only when fill is requested and no epoch is open.
pub(crate) fn validate_initial_fill_admission(
    fill_requested: bool,
    open_epoch: Option<&super::ImportStart>,
    accepted_settlement_exists: Option<bool>,
) -> Result<(), HistoricalReconstructionError> {
    if fill_requested && open_epoch.is_none() && accepted_settlement_exists != Some(true) {
        return Err(HistoricalReconstructionError::EmbeddingFillUnavailable);
    }
    Ok(())
}

/// Prepare the no-endpoint operation, whose fill request cannot settle an open epoch.
pub(crate) fn prepare_without_endpoint(
    frozen: FrozenHistoricalImport,
    fill_requested: bool,
) -> Result<PreparedHistoricalImport, HistoricalReconstructionError> {
    if fill_requested && frozen.epoch {
        return Err(HistoricalReconstructionError::EmbeddingFillUnavailable);
    }
    prepare_frozen(frozen)
}

/// Run native scoring preparation after the operation-specific fill policy passed.
/// Explicit endpoint fill can settle an open epoch, so its caller skips the guard.
pub(crate) fn prepare_frozen(
    frozen: FrozenHistoricalImport,
) -> Result<PreparedHistoricalImport, HistoricalReconstructionError> {
    Ok(frozen.prepare_native()?)
}

/// Build the public report from the prepared durable settlement facts.
pub(crate) fn report(
    prepared: &PreparedHistoricalImport,
    committed: bool,
    embedding: Option<HistoricalEmbeddingReport>,
) -> HistoricalReconstructionReport {
    HistoricalReconstructionReport {
        import_id: prepared.settlement.import_id,
        manifest_digest: prepared.settlement.manifest_digest,
        activity_cut: prepared.settlement.settlement_cut,
        reconstructed_memories: prepared.settlement.records.len(),
        committed,
        embedding,
    }
}
