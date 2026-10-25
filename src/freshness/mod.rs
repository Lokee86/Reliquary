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

#[derive(Clone, Copy)]
pub(crate) enum TraceOperation {
    AcceptedUse,
    InitialDreamPrepare,
    InitialDreamEvaluate,
    InitialDreamCommit,
}

impl TraceOperation {
    fn name(self) -> &'static str {
        match self {
            Self::AcceptedUse => "accepted_use",
            Self::InitialDreamPrepare => "initial_dream_prepare",
            Self::InitialDreamEvaluate => "initial_dream_evaluate",
            Self::InitialDreamCommit => "initial_dream_commit",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TraceStage {
    AcceptedUseReceiptValidation,
    AcceptedUseRootCollection,
    AcceptedUseExactRetryLookup,
    AcceptedUseDirectGraphMaterialization,
    AcceptedUseAdjacencyConstruction,
    AcceptedUseCommunityProofPinning,
    AcceptedUseCommunityMembershipMaterialization,
    AcceptedUsePropagation,
    AcceptedUseEffectPostimagePreparation,
    AcceptedUseContainerAppendSync,
    DreamPropagation,
    DreamDurableHistoricalGraphScan,
    DreamRootCollectionPersistence,
    DreamDuplicateRootResolution,
    DreamHistoricalGraphSnapshot,
    DreamReceiptValidationPostimageAppendSyncDueRefresh,
}

impl TraceStage {
    fn name(self) -> &'static str {
        match self {
            Self::AcceptedUseReceiptValidation => "accepted_use.receipt_validation",
            Self::AcceptedUseRootCollection => "accepted_use.root_collection",
            Self::AcceptedUseExactRetryLookup => "accepted_use.exact_retry_lookup",
            Self::AcceptedUseDirectGraphMaterialization => {
                "accepted_use.direct_graph_materialization"
            }
            Self::AcceptedUseAdjacencyConstruction => "accepted_use.adjacency_construction",
            Self::AcceptedUseCommunityProofPinning => "accepted_use.community_proof_pinning",
            Self::AcceptedUseCommunityMembershipMaterialization => {
                "accepted_use.community_membership_materialization"
            }
            Self::AcceptedUsePropagation => "accepted_use.propagation",
            Self::AcceptedUseEffectPostimagePreparation => {
                "accepted_use.effect_postimage_preparation"
            }
            Self::AcceptedUseContainerAppendSync => "accepted_use.container_append_sync",
            Self::DreamPropagation => "dream.propagation",
            Self::DreamDurableHistoricalGraphScan => "dream.durable_historical_graph_scan",
            Self::DreamRootCollectionPersistence => "dream.root_collection_persistence",
            Self::DreamDuplicateRootResolution => "dream.duplicate_root_resolution",
            Self::DreamHistoricalGraphSnapshot => "dream.historical_graph_snapshot",
            Self::DreamReceiptValidationPostimageAppendSyncDueRefresh => {
                "dream.receipt_validation_postimage_append_sync_due_refresh"
            }
        }
    }
}

/// Own operation-level diagnostic scopes; the store owns only the trace buffer.
pub(crate) struct TraceScope {
    _store_scope: crate::freshness_trace::Scope,
}

pub(crate) fn operation_trace_scope(operation: TraceOperation) -> TraceScope {
    TraceScope {
        _store_scope: crate::freshness_trace::scope(operation.name()),
    }
}

pub(crate) struct TraceStageGuard {
    stage: TraceStage,
    #[cfg(feature = "freshness-performance-trace")]
    started: Option<std::time::Instant>,
}

impl Drop for TraceStageGuard {
    fn drop(&mut self) {
        #[cfg(feature = "freshness-performance-trace")]
        if !std::thread::panicking()
            && let Some(started) = self.started
        {
            crate::freshness_trace::record(
                self.stage.name(),
                started.elapsed().as_nanos(),
                crate::freshness_trace::Counts::default(),
            );
        }
    }
}

/// Time a named Freshness stage without accepting or invoking caller closures.
pub(crate) fn trace_stage(stage: TraceStage) -> TraceStageGuard {
    TraceStageGuard {
        stage,
        #[cfg(feature = "freshness-performance-trace")]
        started: crate::freshness_trace::enabled().then(std::time::Instant::now),
    }
}

pub(crate) fn flush_operation_trace() {
    crate::freshness_trace::flush();
}

#[cfg(feature = "freshness-performance-trace")]
pub(crate) fn record_initial_dream_graph_snapshot(
    elapsed_ns: u128,
    payload_bytes: u64,
    object_bytes: u64,
    chunks: u64,
    mutations: u64,
    communities: u64,
    matched: u64,
    filtered: u64,
) {
    crate::freshness_trace::record(
        "dream.initial_dream_graph_snapshot",
        elapsed_ns,
        crate::freshness_trace::Counts {
            payload_bytes: Some(payload_bytes),
            object_bytes: Some(object_bytes),
            chunks_visited: Some(chunks),
            graph_mutations_decoded: Some(mutations),
            community_snapshots_encountered: Some(communities),
            community_snapshots_matched: Some(matched),
            version_filtered_records: Some(filtered),
            ..Default::default()
        },
    );
}

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
pub(crate) mod accepted_use;
pub(crate) mod graph_snapshot;
