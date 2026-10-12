//! Sparse chronological scoring for the import-only historical projection.
use super::FreshnessPolicy;
use super::score::{FreshnessError, FreshnessRecord};
use crate::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalScoreEvent {
    pub recipient: MemoryId,
    pub position: u64,
    pub event_id: [u8; 32],
    pub amount: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalScoreError {
    InvalidEvent,
    DuplicateEvent([u8; 32]),
    UnknownRecipient(MemoryId),
    Score(FreshnessError),
}

impl std::fmt::Display for HistoricalScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical scoring: {self:?}")
    }
}

impl std::error::Error for HistoricalScoreError {}

impl From<FreshnessError> for HistoricalScoreError {
    fn from(error: FreshnessError) -> Self {
        Self::Score(error)
    }
}

/// Applies recipient contributions in canonical historical order and settles the
/// resulting records through the requested as-of position. Event identity is checked before
/// scoring, so retries cannot apply one source event twice.
pub fn score_historical_records(
    initial: &BTreeMap<MemoryId, FreshnessRecord>,
    events: &[HistoricalScoreEvent],
    as_of_position: u64,
    policy: &FreshnessPolicy,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, HistoricalScoreError> {
    policy
        .validate()
        .map_err(|_| HistoricalScoreError::InvalidEvent)?;
    if as_of_position == 0
        || events.iter().any(|event| {
            event.position == 0
                || event.position > as_of_position
                || !(-100..=100).contains(&event.amount)
                || event.amount == 0
        })
    {
        return Err(HistoricalScoreError::InvalidEvent);
    }
    let mut seen = BTreeSet::new();
    for event in events {
        if !seen.insert(event.event_id) {
            return Err(HistoricalScoreError::DuplicateEvent(event.event_id));
        }
        if !initial.contains_key(&event.recipient) {
            return Err(HistoricalScoreError::UnknownRecipient(event.recipient));
        }
    }

    let mut ordered = events.to_vec();
    ordered.sort_by_key(|event| (event.recipient, event.position, event.event_id));
    let mut records = initial.clone();
    for event in ordered {
        let record = records
            .get_mut(&event.recipient)
            .ok_or(HistoricalScoreError::UnknownRecipient(event.recipient))?;
        if event.amount > 0 {
            record.reinforce_with_policy(event.amount, event.position, policy)?;
        } else {
            record.settle_with_policy(event.position, policy)?;
            record.score = record.score.saturating_add(event.amount).max(-100);
        }
    }
    for record in records.values_mut() {
        record.settle_with_policy(as_of_position, policy)?;
    }
    Ok(records)
}

#[cfg(test)]
#[path = "historical_scorer_tests.rs"]
mod tests;
