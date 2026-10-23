//! Stable service adapter for canonical historical Freshness scoring.
use super::{FreshnessPolicy, FreshnessRecord};
use crate::MemoryId;
use std::collections::BTreeMap;

pub use super::historical_contract::HistoricalScoreEvent;
pub use super::historical_record_scoring::HistoricalScoreError;

/// Apply the canonical storage-owned chronological scorer.
pub fn score_historical_records(
    initial: &BTreeMap<MemoryId, FreshnessRecord>,
    events: &[HistoricalScoreEvent],
    as_of_position: u64,
    policy: &FreshnessPolicy,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, HistoricalScoreError> {
    super::historical_record_scoring::score_historical_records(
        initial,
        events,
        as_of_position,
        policy,
    )
}

#[cfg(test)]
#[path = "historical_scorer_tests.rs"]
mod tests;

/// Prepare immutable numeric authority with the same arithmetic used by decode.
pub(crate) fn prepare_import_numeric_authority(
    settlement: &mut crate::freshness_storage::HistoricalImportFreshnessSettlement,
    initial: &BTreeMap<MemoryId, FreshnessRecord>,
    events: &[HistoricalScoreEvent],
) -> Result<(), String> {
    let basis = super::historical_causal_basis::HistoricalCausalBasis::from_events(
        initial,
        events,
        settlement.settlement_cut,
    )?;
    settlement.records = basis.reconstruct(settlement.settlement_cut)?;
    settlement.causal = Some(basis);
    Ok(())
}

/// Interval recognition runs after full Freshness owner hydration. This checks
/// the complete structural frame without exposing unresolved numeric authority.
pub(crate) fn validate_encoded_settlement_structure(payload: &[u8]) -> Result<(), String> {
    crate::freshness_storage::HistoricalImportFreshnessSettlement::decode_binding(payload)
        .map(|_| ())
}
