//! Deterministic owner-local Memory Freshness policy.
pub(crate) mod historical_causal_basis;
pub(crate) mod historical_contract;
pub mod historical_dream;
pub mod historical_planner;
pub mod historical_propagation;
pub(crate) mod historical_record_scoring;
pub mod historical_scorer;
pub mod historical_semantic;
mod policy;
mod propagation;
mod score;
pub mod semantic_policy;

pub use historical_scorer::{HistoricalScoreError, HistoricalScoreEvent, score_historical_records};
pub use policy::{
    FRESHNESS_POLICY_VERSION, FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN, FreshnessPolicy,
    FreshnessPolicyError,
};
pub(crate) use propagation::propagate_event_candidates_with_policy;
pub use propagation::{
    CommunityMembership, MemoryAdjacency, PropagationError, PropagationEvent, PropagationOutcome,
    PropagationStats, default_worker_count, propagate, propagate_events_with_policy,
    propagate_with_policy, propagate_with_policy_and_stats, propagate_with_stats,
};
pub(crate) use score::settle_with_initial_decay_carry;
pub use score::{
    FreshnessBoundaryIndex, FreshnessError, FreshnessRecord, FreshnessState, next_state_boundary,
    next_state_boundary_with_policy,
};

/// Stable Freshness event identity for an accepted initial Dream pass.
pub(crate) fn initial_dream_pass_identity(owner: [u8; 16], source: crate::MemoryId) -> String {
    crate::dream_pass_identity::initial_pass_id(owner, source)
}

pub(crate) mod reconciliation;

pub(crate) mod historical_settlement;

/// Record immutable evidence-scan counters without exposing the trace store.
#[cfg(feature = "freshness-performance-trace")]
pub(crate) fn record_dream_evidence_scan(
    elapsed_ns: u128,
    payload_bytes: u64,
    object_bytes: u64,
    chunks: u64,
    mutations: u64,
    filtered: u64,
) {
    crate::freshness_trace::record(
        "dream.initial_dream_committed_roots",
        elapsed_ns,
        crate::freshness_trace::Counts {
            payload_bytes: Some(payload_bytes),
            object_bytes: Some(object_bytes),
            chunks_visited: Some(chunks),
            graph_mutations_decoded: Some(mutations),
            version_filtered_records: Some(filtered),
            ..Default::default()
        },
    );
}
