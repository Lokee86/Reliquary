use crate::freshness::FreshnessRecord;
use crate::{Cva, CvaError, MemoryId};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "cva_freshness_r2_tests.rs"]
mod r2_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessEventProof {
    pub graph_version: u64,
    pub community_generation: Option<u64>,
    pub community_graph_version: Option<u64>,
}

/// A precise inclusive cut through the canonical Freshness ledger.
/// Events at the same REL turn are ordered by their stable event identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessEventCut {
    pub rel_turn: u64,
    pub event_id_inclusive: Option<String>,
    pub graph_version: u64,
}

impl Cva {
    pub fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        self.freshness.policy()
    }

    pub fn freshness_has_pending_publication(&self, id: MemoryId) -> bool {
        self.dream_freshness.pending_initialization(id).is_some()
    }

    pub fn freshness_is_publication_birth(&self, id: MemoryId) -> bool {
        self.dream_freshness.is_publication_birth(id)
    }

    pub(crate) fn freshness_has_pending_dream_passes(&self) -> bool {
        self.dream_freshness.has_pending()
    }

    pub fn initial_dream_pass_settled(&self, pass_id: &str) -> bool {
        self.dream_freshness.is_settled(pass_id)
    }

    pub(crate) fn freshness_event_proof_is_available(&self, proof: FreshnessEventProof) -> bool {
        proof.graph_version <= self.graph.memory_graph_version()
            && match (proof.community_generation, proof.community_graph_version) {
                (Some(generation), Some(version)) => {
                    version == proof.graph_version
                        && self.communities.contains_snapshot(generation, version)
                }
                (None, None) => true,
                _ => false,
            }
    }

    pub(crate) fn freshness_prepare_memory_publication(
        &mut self,
        id: MemoryId,
        mutation_id: &str,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), CvaError> {
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "publication turn exceeds accepted owner REL activity".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit Freshness migration".into())
        })?;
        let payload = self
            .dream_freshness
            .publication_intent_payload(id, mutation_id, turn, admit_at_birth)
            .map_err(CvaError::Freshness)?;
        if !payload.is_empty() {
            self.container.append(&payload)?;
            self.container.sync()?;
            self.dream_freshness
                .ingest(&payload)
                .map_err(CvaError::Freshness)?;
        }
        let _ = owner;
        Ok(())
    }

    /// Pins numerical defaults before any Memory or Dream Freshness state is enrolled.
    pub fn set_freshness_policy_before_enrollment(
        &mut self,
        policy: crate::freshness::FreshnessPolicy,
    ) -> Result<(), CvaError> {
        if self.dream_freshness.has_any() || !self.freshness.records().is_empty() {
            return Err(CvaError::Freshness(
                "Freshness policy is immutable after first enrollment".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness(
                "legacy owner requires explicit migration before Freshness policy".into(),
            )
        })?;
        self.freshness
            .set_policy_before_enrollment(policy)
            .map_err(CvaError::Freshness)?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)
    }

    /// Returns the owner-local durable record, including an explicit Web-admission anchor.
    pub fn freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.freshness.records().get(&id).copied()
    }

    pub fn freshness_record_at_cut(
        &self,
        id: MemoryId,
        cut: &FreshnessEventCut,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        if cut.rel_turn > self.rel_turn_count()
            || cut.graph_version > self.memory_graph_version()
            || cut
                .event_id_inclusive
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(CvaError::Freshness(
                "invalid historical Freshness event cut".into(),
            ));
        }
        self.freshness
            .record_at_cut(id, cut)
            .map_err(CvaError::Freshness)
    }

    pub fn freshness_record_at(
        &self,
        id: MemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        if turn > self.rel_turn_count() {
            return Err(CvaError::Freshness(
                "Freshness cut is beyond the REL activity head".into(),
            ));
        }
        self.freshness
            .record_at(id, turn)
            .map_err(CvaError::Freshness)
    }

    /// Returns a projection at an explicit historical owner-REL activity position.
    pub fn freshness_records_at(
        &self,
        turn: u64,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        if turn > self.rel_turn_count() {
            return Err(CvaError::Freshness(
                "Freshness cut is beyond the REL activity head".into(),
            ));
        }
        self.freshness.records_at(turn).map_err(CvaError::Freshness)
    }

    /// Explicit, one-time baseline for an active legacy Memory with no accepted
    /// Freshness birth proof. Starts at the *current* accepted owner REL turn;
    /// it cannot backdate a score or create a first-cycle Dream publication birth.
    /// Repeat calls preserve the original baseline even after further activity.
    pub fn enroll_legacy_freshness_baseline(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.require_freshness_memory(id)?;
        if self.freshness_is_publication_birth(id) || self.freshness_has_pending_publication(id) {
            return Err(CvaError::Freshness(
                "a proven first publication cannot be re-enrolled as a legacy baseline".into(),
            ));
        }
        if self.freshness.records().contains_key(&id) {
            return Ok(false);
        }
        let memory = self.memory(id)?;
        if memory.archived || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
        {
            return Err(CvaError::Freshness(
                "legacy baseline requires a nonarchived settled Memory".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit owner migration first".into())
        })?;
        let turn = self.archive.rel_turn_count();
        let mut record = FreshnessRecord::created();
        record
            .admit(turn)
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        let payload = self.freshness.encode_initialize(owner, id, turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness_refresh_due([id], turn)?;
        Ok(true)
    }

    /// Initializes only an accepted first-publication Memory. Legacy enrollment
    /// must use the explicit baseline API and is never a Dream birth receipt.
    pub fn freshness_initialize(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.require_freshness_memory(id)?;
        let intent = self.dream_freshness.pending_initialization(id);
        if self.freshness.records().contains_key(&id) {
            if let Some((mutation_id, turn, admit_at_birth)) = intent {
                if self.memories.first_mutation_id(id) != Some(mutation_id.as_str()) {
                    return Err(CvaError::Freshness(
                        "publication intent does not match first accepted Memory identity".into(),
                    ));
                }
                // Ordinary reopen may derive a birth from an accepted Memory
                // and pending intent without mutating the file. Persist that
                // derived birth before consuming the only durable recovery
                // proof, or a second reopen would silently lose the score.
                if !self
                    .freshness
                    .has_durable_birth(id)
                    .map_err(CvaError::Freshness)?
                {
                    self.persist_accepted_freshness_birth(id, turn, admit_at_birth)?;
                }
                self.consume_publication_intent(id)?;
            }
            return Ok(false);
        }
        // A normal accepted birth has an immutable first-publication intent.
        // Legacy Memories require a separate explicit migration policy; an
        // ordinary caller must never manufacture a fresh +100 birth for them.
        let (mutation_id, turn, admit_at_birth) = intent.clone().ok_or_else(|| {
            CvaError::Freshness(
                "Freshness enrollment requires accepted first-publication proof; legacy enrollment needs explicit migration".into(),
            )
        })?;
        if self.memories.first_mutation_id(id) != Some(mutation_id.as_str()) {
            return Err(CvaError::Freshness(
                "publication intent does not match first accepted Memory identity".into(),
            ));
        }
        self.persist_accepted_freshness_birth(id, turn, admit_at_birth)?;
        self.freshness_refresh_due([id], turn)?;
        if intent.is_some() {
            self.consume_publication_intent(id)?;
        }
        Ok(true)
    }

    fn persist_accepted_freshness_birth(
        &mut self,
        id: MemoryId,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), CvaError> {
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before enrollment".into())
        })?;
        let mut record = FreshnessRecord::created();
        if admit_at_birth {
            record
                .admit(turn)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
        }
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        let payload = self.freshness.encode_initialize(owner, id, turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)
    }

    fn consume_publication_intent(&mut self, id: MemoryId) -> Result<(), CvaError> {
        if let Some(payload) = self
            .dream_freshness
            .consume_publication_intent_payload(id)
            .map_err(CvaError::Freshness)?
        {
            self.container.append(&payload)?;
            self.container.sync()?;
            self.dream_freshness
                .ingest(&payload)
                .map_err(CvaError::Freshness)?;
        }
        Ok(())
    }

    /// Starts decay at the first successful active Dream settlement. Repeated calls
    /// preserve the first accepted position.
    pub(crate) fn freshness_admit(&mut self, id: MemoryId, turn: u64) -> Result<bool, CvaError> {
        self.require_freshness_memory(id)?;
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "admission turn exceeds accepted owner REL activity".into(),
            ));
        }
        if !self.freshness.records().contains_key(&id) {
            return Err(CvaError::Freshness(
                "Memory must be explicitly enrolled before admission".into(),
            ));
        }
        let current = self.freshness.records()[&id];
        let mut admitted = current;
        if !admitted
            .admit(turn)
            .map_err(|error| CvaError::Freshness(error.to_string()))?
        {
            return Ok(false);
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before admission".into())
        })?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        let payload = self.freshness.encode_admit(owner, id, admitted);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness_refresh_due([id], turn)?;
        Ok(true)
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
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before events".into())
        })?;
        let pass = self.dream_freshness.pending(event_id).ok_or_else(|| {
            CvaError::Freshness("Dream event lacks its accepted initial-pass receipt".into())
        })?;
        if event_id != crate::dream_freshness::initial_pass_id(owner, pass.source_id)
            || pass.accepted_settlement_turn != Some(turn)
            || !self.freshness_is_publication_birth(pass.source_id)
            || !self
                .dream_freshness
                .accepted_graph_cut(event_id)
                .is_some_and(|(_, accepted)| accepted == proof)
            || pass.roots.is_empty()
            || !pass.roots.is_subset(&pass.preexisting_ids)
        {
            return Err(CvaError::Freshness(
                "Dream event lacks matching source, roots, settlement cut, or pinned proof".into(),
            ));
        }
        let cohort = self.freshness_publication_cohort(pass.source_id)?;
        if effects.keys().any(|id| cohort.contains(id)) {
            return Err(CvaError::Freshness(
                "Dream event cannot reinforce its newly created publication cohort".into(),
            ));
        }
        Ok(())
    }

    fn freshness_commit_event(
        &mut self,
        event_id: impl Into<String>,
        turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<bool, CvaError> {
        let event_id = event_id.into();
        self.validate_initial_dream_event_identity(&event_id, turn, proof, &effects)?;
        if self
            .freshness
            .contains_event(&event_id)
            .map_err(CvaError::Freshness)?
        {
            return if self
                .freshness
                .event_matches(&event_id, turn, proof, &effects)
                .map_err(CvaError::Freshness)?
            {
                Ok(false)
            } else {
                Err(CvaError::Freshness(
                    "Freshness event identity collides with different payload".into(),
                ))
            };
        }
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "event turn exceeds accepted owner REL activity".into(),
            ));
        }
        if !self.freshness_event_proof_is_available(proof) {
            return Err(CvaError::Freshness(
                "event graph proof exceeds owner graph head".into(),
            ));
        }
        if proof.community_generation.is_some() != proof.community_graph_version.is_some()
            || proof
                .community_graph_version
                .is_some_and(|version| version > proof.graph_version)
        {
            return Err(CvaError::Freshness(
                "invalid event community snapshot proof".into(),
            ));
        }
        if effects.is_empty() {
            return Ok(false);
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before events".into())
        })?;
        let affected = effects.keys().copied().collect::<Vec<_>>();
        for (id, amount) in &effects {
            self.require_freshness_memory(*id)?;
            if *amount <= 0 || *amount > self.freshness.policy().linkage_principal {
                return Err(CvaError::Freshness(
                    "event effect exceeds the pinned positive principal bound".into(),
                ));
            }
            let _record = *self.freshness.records().get(id).ok_or_else(|| {
                CvaError::Freshness(format!("Memory {:?} has no Freshness enrollment", id))
            })?;
        }
        let postimages = self
            .freshness
            .prepare_event_postimages_for(&event_id, turn, &effects)
            .map_err(CvaError::Freshness)?;
        self.freshness
            .validate_event_payload(&event_id, turn, proof, &effects, &postimages)
            .map_err(CvaError::Freshness)?;
        let payload = self
            .freshness
            .encode_event(owner, event_id, turn, proof, effects, postimages);
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness_refresh_due(affected, turn)?;
        Ok(true)
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
        self.require_freshness_memory(source_id)?;
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before Dream pass".into())
        })?;
        let expected_id = crate::dream_freshness::initial_pass_id(owner, source_id);
        if pass_id != expected_id || !self.freshness_is_publication_birth(source_id) {
            return Err(CvaError::Freshness(
                "Dream pass lacks canonical first-publication identity".into(),
            ));
        }
        if graph_version_before != self.graph_version() {
            return Err(CvaError::Freshness(
                "Dream pass graph cut does not match the accepted graph head".into(),
            ));
        }
        let first = self.memory_revision(source_id, 1)?;
        let current = self.memory(source_id)?;
        if current.archived
            || !(first.lifecycle_state == "extracted" && current.lifecycle_state == "extracted"
                || matches!(first.lifecycle_state.as_str(), "knowledge" | "canonical"))
        {
            return Err(CvaError::Freshness(
                "Memory is not eligible for its initial Dream pass".into(),
            ));
        }
        for id in &preexisting_ids {
            let candidate = self.memory_revision(*id, 1)?;
            if !self.freshness_is_publication_birth(*id)
                || candidate.global_version >= first.global_version
            {
                return Err(CvaError::Freshness(
                    "Dream root lacks pre-existing publication provenance".into(),
                ));
            }
        }
        if first_revision != 1 || preexisting_ids.contains(&source_id) {
            return Err(CvaError::Freshness(
                "invalid first-cycle Dream pass identity".into(),
            ));
        }
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "Dream pass turn exceeds accepted owner REL activity".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before Dream pass".into())
        })?;
        let pass = crate::InitialDreamPass {
            pass_id,
            source_id,
            first_revision,
            turn,
            accepted_settlement_turn: None,
            preexisting_ids,
            graph_version_before,
            roots: std::collections::BTreeSet::new(),
        };
        let payload = self
            .dream_freshness
            .begin_payload(&pass)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return self
                .dream_freshness
                .pending_for_source(source_id)
                .ok_or_else(|| CvaError::Freshness("pass receipt missing".into()));
        }
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)?;
        self.dream_freshness
            .pending_for_source(source_id)
            .ok_or_else(|| CvaError::Freshness("pass receipt missing after append".into()))
    }

    pub fn pending_initial_dream_pass(
        &self,
        source_id: MemoryId,
    ) -> Option<crate::InitialDreamPass> {
        self.dream_freshness.pending_for_source(source_id)
    }

    pub(crate) fn accept_initial_dream_settlement(
        &mut self,
        pass_id: &str,
        turn: u64,
    ) -> Result<bool, CvaError> {
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "settlement turn exceeds accepted owner REL activity".into(),
            ));
        }
        if let Some(pass) = self.dream_freshness.pending(pass_id)
            && pass.accepted_settlement_turn.is_some()
        {
            return if pass.accepted_settlement_turn == Some(turn) {
                Ok(false)
            } else {
                Err(CvaError::Freshness(
                    "Dream accepted settlement cut cannot change".into(),
                ))
            };
        }
        let snapshot = self
            .community_snapshot()
            .filter(|snapshot| snapshot.derived_graph_version == self.memory_graph_version());
        let proof = FreshnessEventProof {
            graph_version: self.memory_graph_version(),
            community_generation: snapshot.as_ref().map(|snapshot| snapshot.generation),
            community_graph_version: snapshot
                .as_ref()
                .map(|snapshot| snapshot.derived_graph_version),
        };
        let payload = self
            .dream_freshness
            .accept_graph_payload(pass_id, turn, self.graph_version(), proof)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(false);
        }
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn record_initial_dream_roots(
        &mut self,
        pass_id: &str,
        roots: std::collections::BTreeSet<MemoryId>,
    ) -> Result<(), CvaError> {
        let payload = self
            .dream_freshness
            .roots_payload(pass_id, &roots)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)
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
        if self.dream_freshness.is_settled(pass_id) {
            return Ok(false);
        }
        let pass = self
            .dream_freshness
            .pending(pass_id)
            .ok_or_else(|| CvaError::Freshness("Dream pass is not pending".into()))?;
        let turn = pass.accepted_settlement_turn.ok_or_else(|| {
            CvaError::Freshness("Dream settlement acceptance cut is missing".into())
        })?;
        if !self.freshness_event_proof_is_available(proof) {
            return Err(CvaError::Freshness(
                "Dream event graph proof is unavailable".into(),
            ));
        }
        if self
            .dream_freshness
            .accepted_graph_cut(pass_id)
            .is_some_and(|(_, accepted)| accepted != proof)
        {
            return Err(CvaError::Freshness(
                "Dream event proof differs from accepted settlement snapshot".into(),
            ));
        }
        if !effects.is_empty() {
            self.validate_initial_dream_event_identity(pass_id, turn, proof, &effects)?;
        }
        if !effects.is_empty()
            && !self
                .freshness
                .event_matches(pass_id, turn, proof, &effects)
                .map_err(CvaError::Freshness)?
        {
            if self
                .freshness
                .contains_event(pass_id)
                .map_err(CvaError::Freshness)?
            {
                return Err(CvaError::Freshness(
                    "Dream pass event identity collides with different payload".into(),
                ));
            }
            let postimages = self
                .freshness
                .prepare_event_postimages_for(pass_id, turn, &effects)
                .map_err(CvaError::Freshness)?;
            self.freshness
                .validate_event_payload(pass_id, turn, proof, &effects, &postimages)
                .map_err(CvaError::Freshness)?;
        }
        if active_admission {
            let _ = self.freshness_admit(pass.source_id, turn)?;
        }
        if !effects.is_empty() {
            let _ = self.freshness_commit_event(pass.pass_id.clone(), turn, proof, effects)?;
        }
        let payload = self
            .dream_freshness
            .settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn freshness_replay_entries(
        &self,
    ) -> Result<Vec<crate::freshness_storage::FreshnessReplayEntry>, CvaError> {
        self.freshness.replay_entries().map_err(CvaError::Freshness)
    }

    pub(crate) fn freshness_publication_cohort(
        &self,
        source: MemoryId,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut cohort = std::collections::BTreeSet::from([source]);
        scan.visit_payloads::<CvaError>(|_, payload| {
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|message| CvaError::Freshness(message.into()))?
                && completion.freshness_birth_turn.is_some()
                && completion.freshness_birth_memory_ids.contains(&source)
            {
                cohort.extend(completion.freshness_birth_memory_ids);
            }
            Ok(())
        })?;
        Ok(cohort)
    }

    pub(crate) fn freshness_same_publication_cohort(
        &self,
        a: MemoryId,
        b: MemoryId,
    ) -> Result<bool, CvaError> {
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut same = false;
        scan.visit_payloads::<CvaError>(|_, payload| {
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|message| CvaError::Freshness(message.into()))?
            {
                same |= completion.freshness_birth_turn.is_some()
                    && completion.freshness_birth_memory_ids.contains(&a)
                    && completion.freshness_birth_memory_ids.contains(&b);
            }
            Ok(())
        })?;
        Ok(same)
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
        let (cut, proof) = self
            .dream_freshness
            .accepted_graph_cut(pass_id)
            .ok_or_else(|| {
                CvaError::Freshness("Dream acceptance lacks durable graph provenance".into())
            })?;
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut objects = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut states = BTreeMap::new();
        let mut memberships = crate::freshness::CommunityMembership::new();
        let mut community_found = proof.community_generation.is_none();
        scan.visit_payloads::<CvaError>(|_, payload| {
            if let Some(snapshot) = crate::community_codec::decode_snapshot(payload)? {
                if Some(snapshot.generation) == proof.community_generation
                    && Some(snapshot.derived_graph_version) == proof.community_graph_version
                {
                    community_found = true;
                    for community in snapshot.communities {
                        for memory in community.members {
                            memberships.insert(memory, community.id);
                        }
                    }
                }
            }
            let Some(version) = crate::graph_codec::decode_version(payload)? else {
                return Ok(());
            };
            if version.graph_version > cut {
                return Ok(());
            }
            let payload = objects.read(version.mutation)?;
            let mutations = if let Some(batch) = crate::graph_codec::decode_batch(&payload)? {
                batch
            } else if let Some(mutation) = crate::graph_codec::decode_mutation(&payload)? {
                vec![mutation]
            } else {
                return Err(CvaError::Freshness(
                    "Dream graph snapshot lacks committed mutation".into(),
                ));
            };
            for mutation in mutations {
                let (Some(source), Some(target)) =
                    (mutation.source.as_memory(), mutation.target.as_memory())
                else {
                    continue;
                };
                states.insert((source, target, mutation.kind), mutation.active);
            }
            Ok(())
        })?;
        if !community_found {
            return Err(CvaError::Freshness(
                "Dream accepted Community snapshot is unavailable".into(),
            ));
        }
        let mut adjacency = crate::freshness::MemoryAdjacency::new();
        for ((source, target, _), active) in states {
            if active {
                adjacency.entry(source).or_default().insert(target);
                adjacency.entry(target).or_default().insert(source);
            }
        }
        Ok((proof, adjacency, memberships))
    }

    pub(crate) fn initial_dream_committed_roots(
        &self,
        pass: &crate::InitialDreamPass,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut objects = crate::Container::open_read_only_for_scan(self.container.path())?;
        let mut roots = std::collections::BTreeSet::new();
        scan.visit_payloads::<CvaError>(|_, payload| {
            let Some(version) = crate::graph_codec::decode_version(payload)? else {
                return Ok(());
            };
            let end = self
                .dream_freshness
                .accepted_graph_cut(&pass.pass_id)
                .map(|(graph, _)| graph)
                .unwrap_or(self.graph_version());
            if version.graph_version <= pass.graph_version_before || version.graph_version > end {
                return Ok(());
            }
            let payload = objects.read(version.mutation)?;
            let mutations = if let Some(batch) = crate::graph_codec::decode_batch(&payload)? {
                batch
            } else if let Some(mutation) = crate::graph_codec::decode_mutation(&payload)? {
                vec![mutation]
            } else {
                return Err(CvaError::Freshness(
                    "committed Dream graph version lacks its mutation".into(),
                ));
            };
            for mutation in mutations {
                if mutation.origin != crate::GraphRelationOrigin::Dream || !mutation.active {
                    continue;
                }
                let (Some(source), Some(target)) =
                    (mutation.source.as_memory(), mutation.target.as_memory())
                else {
                    continue;
                };
                let neighbor = if source == pass.source_id {
                    Some(target)
                } else if target == pass.source_id {
                    Some(source)
                } else {
                    None
                };
                if let Some(neighbor) = neighbor.filter(|id| pass.preexisting_ids.contains(id)) {
                    roots.insert(neighbor);
                }
            }
            Ok(())
        })?;
        Ok(roots)
    }

    pub(crate) fn recover_committed_initial_dream_event(
        &mut self,
        pass_id: &str,
        active_admission: bool,
    ) -> Result<bool, CvaError> {
        let Some((turn, _, _)) = self
            .freshness
            .event_receipt(pass_id)
            .map_err(CvaError::Freshness)?
        else {
            return Ok(false);
        };
        let pass = self.dream_freshness.pending(pass_id).ok_or_else(|| {
            CvaError::Freshness("committed Dream event has no pending pass".into())
        })?;
        if pass.accepted_settlement_turn != Some(turn) {
            return Err(CvaError::Freshness(
                "committed Dream event differs from accepted settlement cut".into(),
            ));
        }
        if active_admission {
            self.freshness_admit(pass.source_id, turn)?;
        }
        let payload = self
            .dream_freshness
            .settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn freshness_replay_initialize(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
        admitted_at_birth: bool,
    ) -> Result<(), CvaError> {
        self.require_freshness_memory(id)?;
        if self.freshness.records().contains_key(&id) {
            return Ok(());
        }
        if destination_turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "replay initialization turn exceeds destination REL activity".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner cannot replay Freshness history".into())
        })?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        let mut record = FreshnessRecord::created();
        if admitted_at_birth {
            record
                .admit(destination_turn)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
        }
        let payload = self
            .freshness
            .encode_initialize(owner, id, destination_turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn freshness_replay_admit(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
    ) -> Result<(), CvaError> {
        let _ = self.freshness_admit(id, destination_turn)?;
        Ok(())
    }

    pub(crate) fn freshness_replay_event(
        &mut self,
        event_id: String,
        destination_turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        provenance: Option<crate::freshness_storage::FreshnessEventProvenance>,
    ) -> Result<(), CvaError> {
        if destination_turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "replay event turn exceeds destination REL activity".into(),
            ));
        }
        if self
            .freshness
            .contains_event(&event_id)
            .map_err(CvaError::Freshness)?
        {
            return if self
                .freshness
                .event_matches_with_provenance(
                    &event_id,
                    destination_turn,
                    proof,
                    &effects,
                    provenance.as_ref(),
                )
                .map_err(CvaError::Freshness)?
            {
                Ok(())
            } else {
                Err(CvaError::Freshness(
                    "replayed event identity collides with different payload".into(),
                ))
            };
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner cannot replay Freshness history".into())
        })?;
        for (id, amount) in &effects {
            self.require_freshness_memory(*id)?;
            if *amount <= 0 || *amount > self.freshness.policy().linkage_principal {
                return Err(CvaError::Freshness(
                    "replayed event effect exceeds principal bound".into(),
                ));
            }
            let _ = self.freshness.records().get(id).ok_or_else(|| {
                CvaError::Freshness(format!("Memory {:?} has no Freshness enrollment", id))
            })?;
        }
        let postimages = self
            .freshness
            .prepare_event_postimages_for(&event_id, destination_turn, &effects)
            .map_err(CvaError::Freshness)?;
        self.freshness
            .validate_event_payload(&event_id, destination_turn, proof, &effects, &postimages)
            .map_err(CvaError::Freshness)?;
        let payload = self
            .freshness
            .encode_event_with_provenance(
                owner,
                event_id,
                destination_turn,
                proof,
                effects.clone(),
                postimages,
                provenance,
            )
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness_refresh_due(effects.keys().copied(), destination_turn)?;
        Ok(())
    }

    pub(crate) fn freshness_replay_accepted_use_receipt(
        &mut self,
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    ) -> Result<(), CvaError> {
        if self
            .freshness
            .resolve_accepted_use(&use_id, turn, &sources)
            .map_err(CvaError::Freshness)?
            .is_some()
        {
            return Ok(());
        }
        let owner = self
            .owner_uuid()
            .ok_or_else(|| CvaError::Freshness("accepted-use replay lacks owner".into()))?;
        let payload = self.freshness.encode_accepted_use_receipt(
            owner,
            use_id,
            turn,
            sources,
            policy_version,
        );
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn freshness_publication_birth_ids(&self) -> Vec<MemoryId> {
        self.dream_freshness.publication_birth_ids()
    }

    pub(crate) fn freshness_replay_publication_birth(
        &mut self,
        id: MemoryId,
    ) -> Result<(), CvaError> {
        self.require_freshness_memory(id)?;
        let payload = self
            .dream_freshness
            .replay_publication_birth_payload(id)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(());
        }
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn freshness_settled_dream_pass_ids(&self) -> Vec<String> {
        self.dream_freshness
            .settled_ids()
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn freshness_replay_settled_dream_pass(
        &mut self,
        pass_id: &str,
    ) -> Result<(), CvaError> {
        let payload = self
            .dream_freshness
            .replay_settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(());
        }
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest(&payload)
            .map_err(CvaError::Freshness)
    }

    fn require_freshness_memory(&self, id: MemoryId) -> Result<(), CvaError> {
        if self.memories.contains_memory(id) {
            Ok(())
        } else {
            Err(CvaError::Freshness(format!(
                "unknown owner-local Memory {:?}",
                id
            )))
        }
    }
}

#[cfg(test)]
#[path = "freshness_recovery_tests.rs"]
mod recovery_tests;
