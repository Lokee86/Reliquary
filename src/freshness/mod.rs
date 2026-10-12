//! Deterministic owner-local Memory Freshness policy.
pub mod historical_checkpoint;
pub mod historical_dream;
pub mod historical_planner;
pub mod historical_propagation;
pub mod historical_scorer;
pub mod historical_semantic;
mod policy;
mod propagation;
mod score;
pub mod semantic_policy;

pub use historical_checkpoint::{
    HistoricalCheckpoint, HistoricalCheckpointCommit, HistoricalCheckpointError,
    HistoricalCheckpointStore,
};
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
pub use score::{
    FreshnessBoundaryIndex, FreshnessError, FreshnessRecord, FreshnessState, next_state_boundary,
    next_state_boundary_with_policy,
};
