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
