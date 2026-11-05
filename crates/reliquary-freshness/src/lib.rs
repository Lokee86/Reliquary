//! Deterministic scoring, propagation, historical evaluation and derived due policy.
//! This boundary is under construction.

mod freshness_due;
pub mod historical_dream;
pub mod historical_import;
pub mod historical_planner;
pub mod historical_propagation;
pub mod historical_scorer;
pub mod historical_semantic;
mod propagation;
mod score;
pub mod semantic_policy;

pub use propagation::{
    CommunityMembership, MemoryAdjacency, PropagationError, PropagationEvent, PropagationOutcome,
    PropagationStats, default_worker_count, propagate, propagate_event_candidates_with_policy,
    propagate_events_with_policy, propagate_with_policy, propagate_with_policy_and_stats,
    propagate_with_stats,
};

pub use freshness_due::{
    FreshnessDue, FreshnessDueDispatcher, FreshnessRefresh, FreshnessReversal,
};
pub use score::{
    FreshnessBoundaryIndex, FreshnessError, FreshnessRecord, FreshnessState, next_state_boundary,
    next_state_boundary_with_policy,
};

pub(crate) use reliquary_freshness_store::settle_with_initial_decay_carry;
pub use reliquary_freshness_store::{
    FRESHNESS_POLICY_VERSION, FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN, FreshnessPolicy,
    FreshnessPolicyError,
};
