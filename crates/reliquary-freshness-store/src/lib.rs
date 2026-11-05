//! Owner-scoped Freshness policy, activity receipts, and score projections.

mod codec;
mod historical_snapshot;
mod model;
mod policy;
mod projection;
mod score;
mod store;

pub use historical_snapshot::HistoricalFreshnessSnapshot;
pub use model::FreshnessMemoryId;
pub use policy::{
    FRESHNESS_POLICY_VERSION, FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN, FreshnessPolicy,
    FreshnessPolicyError,
};
pub use projection::{
    FreshnessEvent, FreshnessEventCut, FreshnessEventProof, FreshnessEventProvenance,
    FreshnessReplayEntry, FreshnessUseReceipt,
};
pub use score::{
    FreshnessError, FreshnessRecord, FreshnessState, settle_with_initial_decay_carry, state_for,
    validate_policy,
};
pub use store::FreshnessStore;

/// Validate a current-format journal frame encountered during owner replay.
/// Score and event ordering are validated by `FreshnessStore` itself.
pub fn validate_journal_frame(bytes: &[u8], owner: [u8; 16]) -> Result<(), String> {
    match codec::decode_entry(bytes)? {
        Some((actual, _)) if actual == owner => Ok(()),
        Some(_) => Err("Freshness journal owner mismatch".into()),
        None => Err("not a Freshness journal frame".into()),
    }
}

#[cfg(test)]
mod tests;
