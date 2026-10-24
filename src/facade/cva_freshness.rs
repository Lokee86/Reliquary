use crate::freshness::FreshnessRecord;
use crate::{Cva, CvaError, FreshnessEventCut, FreshnessEventProof, MemoryId};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "graph_compact_freshness_tests.rs"]
mod graph_compact_tests;
#[cfg(test)]
#[path = "cva_freshness_r2_tests.rs"]
mod r2_tests;

impl Cva {
    pub fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        self.native_freshness_policy()
    }

    pub fn freshness_has_pending_publication(&self, id: MemoryId) -> bool {
        self.native_freshness_has_pending_publication(id)
    }

    pub fn freshness_is_publication_birth(&self, id: MemoryId) -> bool {
        self.native_freshness_is_publication_birth(id)
    }

    pub(crate) fn freshness_has_pending_dream_passes(&self) -> bool {
        self.native_freshness_has_pending_dream_passes()
    }

    pub fn initial_dream_pass_settled(&self, pass_id: &str) -> bool {
        self.native_initial_dream_pass_settled(pass_id)
    }

    pub(crate) fn freshness_event_proof_is_available(&self, proof: FreshnessEventProof) -> bool {
        self.native_freshness_event_proof_is_available(proof)
    }

    /// Pins numerical defaults before any Memory or Dream Freshness state is enrolled.
    pub fn set_freshness_policy_before_enrollment(
        &mut self,
        policy: crate::freshness::FreshnessPolicy,
    ) -> Result<(), CvaError> {
        self.native_set_freshness_policy_before_enrollment(policy)
    }

    /// Returns the owner-local durable record, including an explicit Web-admission anchor.
    pub fn freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.native_freshness_record(id)
    }

    pub fn freshness_record_at_cut(
        &self,
        id: MemoryId,
        cut: &FreshnessEventCut,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        self.native_freshness_record_at_cut(id, cut)
    }

    pub fn freshness_record_at(
        &self,
        id: MemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        self.native_freshness_record_at(id, turn)
    }

    /// Returns a projection at an explicit historical owner-REL activity position.
    pub fn freshness_records_at(
        &self,
        turn: u64,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        self.native_freshness_records_at(turn)
    }

    /// Explicit, one-time baseline for an active legacy Memory with no accepted
    /// Freshness birth proof. Starts at the *current* accepted owner REL turn;
    /// it cannot backdate a score or create a first-cycle Dream publication birth.
    /// Repeat calls preserve the original baseline even after further activity.
    pub fn enroll_legacy_freshness_baseline(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.native_enroll_legacy_freshness_baseline(id)
    }

    /// Initializes only an accepted first-publication Memory. Legacy enrollment
    /// must use the explicit baseline API and is never a Dream birth receipt.
    pub fn freshness_initialize(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.native_freshness_initialize(id)
    }

    /// Starts decay at the first successful active Dream settlement. Repeated calls
    /// preserve the first accepted position.
    pub(crate) fn freshness_admit(&mut self, id: MemoryId, turn: u64) -> Result<bool, CvaError> {
        self.native_freshness_admit(id, turn)
    }

    /// Applies a complete event's per-Memory positive candidate maxima as one durable
    /// owner-local journal chunk. Duplicate identities are harmless retries.
    /// A Dream event cannot be minted from an arbitrary pass ID or arbitrary
    /// source/cohort targets. Call this before *any* settlement admission write:
    /// rejecting an invalid event after admitting its source would leave a
    /// partially accepted failed operation.
    fn validate_initial_dream_event_identity(
        &self,
        event_id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &BTreeMap<MemoryId, i16>,
    ) -> Result<(), CvaError> {
        self.native_validate_initial_dream_event_identity(event_id, turn, proof, effects)
    }

    fn freshness_commit_event(
        &mut self,
        event_id: impl Into<String>,
        turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<bool, CvaError> {
        self.native_freshness_commit_event(event_id, turn, proof, effects)
    }

    /// Durably freezes the first Dream pass identity and clock cut before graph publication.
    pub(crate) fn begin_initial_dream_pass(
        &mut self,
        pass_id: String,
        source_id: MemoryId,
        first_revision: u64,
        turn: u64,
        preexisting_ids: std::collections::BTreeSet<MemoryId>,
        graph_version_before: u64,
    ) -> Result<crate::InitialDreamPass, CvaError> {
        self.native_begin_initial_dream_pass(
            pass_id,
            source_id,
            first_revision,
            turn,
            preexisting_ids,
            graph_version_before,
        )
    }

    pub fn pending_initial_dream_pass(
        &self,
        source_id: MemoryId,
    ) -> Option<crate::InitialDreamPass> {
        self.native_pending_initial_dream_pass(source_id)
    }

    pub(crate) fn accept_initial_dream_settlement(
        &mut self,
        pass_id: &str,
        turn: u64,
    ) -> Result<bool, CvaError> {
        self.native_accept_initial_dream_settlement(pass_id, turn)
    }

    pub(crate) fn record_initial_dream_roots(
        &mut self,
        pass_id: &str,
        roots: std::collections::BTreeSet<MemoryId>,
    ) -> Result<(), CvaError> {
        self.native_record_initial_dream_roots(pass_id, roots)
    }

    /// Admits the source and commits at most one complete first-cycle event using the
    /// original turn and pass ID retained in the owner-local journal.
    pub(crate) fn settle_initial_dream_pass(
        &mut self,
        pass_id: &str,
        active_admission: bool,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<bool, CvaError> {
        self.native_settle_initial_dream_pass(pass_id, active_admission, proof, effects)
    }

    pub(crate) fn freshness_replay_entries(
        &self,
    ) -> Result<Vec<crate::freshness_storage::FreshnessReplayEntry>, CvaError> {
        self.native_freshness_replay_entries()
    }

    pub(crate) fn freshness_publication_cohort(
        &self,
        source: MemoryId,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        self.native_freshness_publication_cohort(source)
    }

    pub(crate) fn freshness_same_publication_cohort(
        &self,
        a: MemoryId,
        b: MemoryId,
    ) -> Result<bool, CvaError> {
        self.native_freshness_same_publication_cohort(a, b)
    }

    pub(crate) fn initial_dream_graph_snapshot(
        &self,
        pass_id: &str,
    ) -> Result<
        (
            FreshnessEventProof,
            crate::freshness::MemoryAdjacency,
            crate::freshness::CommunityMembership,
        ),
        CvaError,
    > {
        self.native_initial_dream_graph_snapshot(pass_id)
    }

    pub(crate) fn initial_dream_committed_roots(
        &self,
        pass: &crate::InitialDreamPass,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        self.native_initial_dream_committed_roots(pass)
    }

    pub(crate) fn initial_dream_committed_evidence(
        &self,
        pass: &crate::InitialDreamPass,
    ) -> Result<(std::collections::BTreeSet<MemoryId>, bool), CvaError> {
        self.native_initial_dream_committed_evidence(pass)
    }

    pub(crate) fn recover_committed_initial_dream_event(
        &mut self,
        pass_id: &str,
        active_admission: bool,
    ) -> Result<bool, CvaError> {
        self.native_recover_committed_initial_dream_event(pass_id, active_admission)
    }

    pub(crate) fn freshness_replay_initialize(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
        admitted_at_birth: bool,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_initialize(id, destination_turn, admitted_at_birth)
    }

    pub(crate) fn freshness_replay_admit(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_admit(id, destination_turn)
    }

    pub(crate) fn freshness_replay_event(
        &mut self,
        event_id: String,
        destination_turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        provenance: Option<crate::freshness_storage::FreshnessEventProvenance>,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_event(event_id, destination_turn, proof, effects, provenance)
    }

    pub(crate) fn freshness_replay_accepted_use_receipt(
        &mut self,
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_accepted_use_receipt(use_id, turn, sources, policy_version)
    }

    pub(crate) fn freshness_replay_semantic_event(
        &mut self,
        event_id: String,
        evidence: crate::freshness_storage::SemanticEventEvidence,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_semantic_event(event_id, evidence, effects)
    }

    fn refresh_freshness_replay_due(
        &mut self,
        outcome: Option<crate::owner_lifecycle::freshness_reconciliation::RefreshDue>,
    ) -> Result<(), CvaError> {
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn freshness_publication_birth_ids(&self) -> Vec<MemoryId> {
        self.native_freshness_publication_birth_ids()
    }

    pub(crate) fn freshness_replay_publication_birth(
        &mut self,
        id: MemoryId,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_publication_birth(id)
    }

    pub(crate) fn freshness_settled_dream_pass_ids(&self) -> Vec<String> {
        self.native_freshness_settled_dream_pass_ids()
    }

    pub(crate) fn freshness_replay_settled_dream_pass(
        &mut self,
        pass_id: &str,
    ) -> Result<(), CvaError> {
        self.native_freshness_replay_settled_dream_pass(pass_id)
    }
}

#[cfg(test)]
#[path = "freshness_recovery_tests.rs"]
mod recovery_tests;

#[cfg(feature = "freshness-performance-trace")]
pub(crate) fn record_dream_evidence_trace(
    trace: crate::history::dream_evidence::DreamEvidenceTrace,
) {
    crate::freshness::record_dream_evidence_scan(
        trace.elapsed_ns,
        trace.payload_bytes,
        trace.object_bytes,
        trace.chunks,
        trace.mutations,
        trace.filtered,
    );
}
