//! Deterministic Freshness service algorithms over caller-supplied owner snapshots.

pub mod historical_dream;
pub mod historical_planner;
pub mod historical_propagation;
pub mod historical_scorer;
pub mod historical_semantic;
mod propagation;
pub mod semantic_policy;

pub use reliquary_freshness_store::{FreshnessPolicy, FreshnessPolicyError};

pub use propagation::{
    CommunityMembership, MemoryAdjacency, PropagationError, PropagationEvent, PropagationOutcome,
    PropagationStats, default_worker_count, propagate, propagate_events_with_policy,
    propagate_with_policy, propagate_with_policy_and_stats, propagate_with_stats,
};

mod freshness_due;
mod score;
pub use freshness_due::{
    FreshnessDue, FreshnessDueDispatcher, FreshnessRefresh, FreshnessReversal,
};
pub use historical_scorer::{HistoricalScoreError, HistoricalScoreEvent, score_historical_records};
pub use propagation::propagate_event_candidates_with_policy;
pub use reliquary_freshness_store::{
    FRESHNESS_POLICY_VERSION, FRESHNESS_SCORE_MAX, FRESHNESS_SCORE_MIN,
};
pub use score::{
    FreshnessBoundaryIndex, FreshnessError, FreshnessRecord, FreshnessState, next_state_boundary,
    next_state_boundary_with_policy,
};

pub(crate) use reliquary_freshness_store::settle_with_initial_decay_carry;
