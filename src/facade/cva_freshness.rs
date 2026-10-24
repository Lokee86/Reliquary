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
        self.freshness.policy()
    }

    pub fn freshness_has_pending_publication(&self, id: MemoryId) -> bool {
        self.runtime_has_pending_publication(id)
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

    /// Pins numerical defaults before any Memory or Dream Freshness state is enrolled.
    pub fn set_freshness_policy_before_enrollment(
        &mut self,
        policy: crate::freshness::FreshnessPolicy,
    ) -> Result<(), CvaError> {
        self.container.require_write_admission()?;
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
        self.owner
            .freshness
            .persist_policy_if_needed(&mut self.owner.container, owner)
            .map_err(CvaError::Freshness)
    }

    /// Returns the owner-local durable record, including an explicit Web-admission anchor.
    pub fn freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.owner
            .freshness
            .current_rewrite_record(id, self.owner.archive.rel_turn_count())
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
        self.runtime_require_freshness_memory(id)?;
        if self.owner.dream_freshness.is_publication_birth(id)
            || self.runtime_has_pending_publication(id)
        {
            return Err(CvaError::Freshness(
                "a proven first publication cannot be re-enrolled as a legacy baseline".into(),
            ));
        }
        if self.freshness.records().contains_key(&id) {
            return Ok(false);
        }
        let memory = self.owner.memories.memory(&self.owner.container, id)?;
        if memory.archived || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical")
        {
            return Err(CvaError::Freshness(
                "legacy baseline requires a nonarchived settled Memory".into(),
            ));
        }
        let owner = self.owner.container.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit owner migration first".into())
        })?;
        let turn = self.owner.archive.rel_turn_count();
        let mut record = FreshnessRecord::created();
        record
            .admit(turn)
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        self.owner
            .freshness
            .persist_policy_if_needed(&mut self.owner.container, owner)
            .map_err(CvaError::Freshness)?;
        let payload = self.freshness.encode_initialize(owner, id, turn, record);
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.runtime_freshness_refresh_due([id], turn)?;
        Ok(true)
    }

    /// Initializes only an accepted first-publication Memory. Legacy enrollment
    /// must use the explicit baseline API and is never a Dream birth receipt.
    pub fn freshness_initialize(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.runtime_freshness_initialize(id)
    }

    /// Starts decay at the first successful active Dream settlement. Repeated calls
    /// preserve the first accepted position.
    pub(crate) fn freshness_admit(&mut self, id: MemoryId, turn: u64) -> Result<bool, CvaError> {
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::admit(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            id,
            turn,
        )?;
        let changed = outcome.is_some();
        self.refresh_freshness_replay_due(outcome)?;
        Ok(changed)
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
        if event_id != crate::dream_pass_identity::initial_pass_id(owner, pass.source_id)
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
            self.runtime_require_freshness_memory(*id)?;
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
        self.owner
            .freshness
            .persist_policy_if_needed(&mut self.owner.container, owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.runtime_freshness_refresh_due(affected, turn)?;
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
        self.runtime_require_freshness_memory(source_id)?;
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before Dream pass".into())
        })?;
        let expected_id = crate::dream_pass_identity::initial_pass_id(owner, source_id);
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
        if self
            .dream_freshness
            .begin_is_retry(&pass)
            .map_err(CvaError::Freshness)?
        {
            return self
                .dream_freshness
                .pending_for_source(source_id)
                .ok_or_else(|| CvaError::Freshness("pass receipt missing".into()));
        }
        let payload = self.current_dream_begin_payload(&pass)?;
        self.owner
            .freshness
            .persist_policy_if_needed(&mut self.owner.container, owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.dream_freshness
            .ingest_materialized_begin(owner, pass)
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
        let history = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        crate::history::publication_birth_cohort(&history, source)
    }

    pub(crate) fn freshness_same_publication_cohort(
        &self,
        a: MemoryId,
        b: MemoryId,
    ) -> Result<bool, CvaError> {
        let history = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        crate::history::same_publication_birth_cohort(&history, a, b)
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
        let mut scan = self.owner.container.read_source()?;
        let mut objects = self.owner.container.read_source()?;
        let mut states = BTreeMap::new();
        let mut memberships = crate::freshness::CommunityMembership::new();
        let mut community_found = proof.community_generation.is_none();
        #[cfg(feature = "freshness-performance-trace")]
        let trace_started = std::time::Instant::now();
        #[cfg(feature = "freshness-performance-trace")]
        let (
            mut trace_payload_bytes,
            mut trace_object_bytes,
            mut trace_chunks,
            mut trace_mutations,
            mut trace_communities,
            mut trace_community_matches,
            mut trace_filtered,
        ) = (0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64, 0_u64);
        {
            let mut ingest_frame =
                |frame: crate::container::PhysicalFrame| -> Result<(), CvaError> {
                    let object = frame.object;
                    let payload = frame.payload.as_slice();
                    {
                        #[cfg(feature = "freshness-performance-trace")]
                        {
                            trace_payload_bytes += payload.len() as u64;
                            trace_chunks += 1;
                        }
                        if let Some(snapshot) = crate::community_codec::decode_snapshot(payload)? {
                            #[cfg(feature = "freshness-performance-trace")]
                            {
                                trace_communities += 1;
                            }
                            if Some(snapshot.generation) == proof.community_generation
                                && Some(snapshot.derived_graph_version)
                                    == proof.community_graph_version
                            {
                                #[cfg(feature = "freshness-performance-trace")]
                                {
                                    trace_community_matches += 1;
                                }
                                community_found = true;
                                for community in snapshot.communities {
                                    for memory in community.members {
                                        memberships.insert(memory, community.id);
                                    }
                                }
                            }
                        }
                        let Some(version) =
                            crate::graph_compact_codec::publication::version_at(object, payload)?
                        else {
                            return Ok(());
                        };
                        if version.graph_version > cut {
                            #[cfg(feature = "freshness-performance-trace")]
                            {
                                trace_filtered += 1;
                            }
                            return Ok(());
                        }
                        let payload = objects.read(version.mutation)?;
                        #[cfg(feature = "freshness-performance-trace")]
                        {
                            trace_object_bytes += payload.len() as u64;
                        }
                        let mutations =
                            self.graph
                                .decode_stored_mutations(&payload)?
                                .ok_or_else(|| {
                                    CvaError::Freshness(
                                        "Dream graph snapshot lacks committed mutation".into(),
                                    )
                                })?;
                        #[cfg(feature = "freshness-performance-trace")]
                        {
                            trace_mutations += mutations.len() as u64;
                        }
                        for mutation in mutations {
                            let (Some(source), Some(target)) =
                                (mutation.source.as_memory(), mutation.target.as_memory())
                            else {
                                continue;
                            };
                            states.insert((source, target, mutation.kind), mutation.active);
                        }
                        Ok(())
                    }
                };
            let mut cursor = scan.physical_cursor()?;
            (|| {
                while let Some(frame) = cursor.next_frame()? {
                    ingest_frame(frame)?;
                }
                Ok::<(), CvaError>(())
            })()
        }?;
        #[cfg(feature = "freshness-performance-trace")]
        crate::freshness_trace::record(
            "dream.initial_dream_graph_snapshot",
            trace_started.elapsed().as_nanos(),
            crate::freshness_trace::Counts {
                payload_bytes: Some(trace_payload_bytes),
                object_bytes: Some(trace_object_bytes),
                chunks_visited: Some(trace_chunks),
                graph_mutations_decoded: Some(trace_mutations),
                community_snapshots_encountered: Some(trace_communities),
                community_snapshots_matched: Some(trace_community_matches),
                version_filtered_records: Some(trace_filtered),
                ..Default::default()
            },
        );
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
        self.initial_dream_committed_evidence(pass)
            .map(|(roots, _)| roots)
    }

    pub(crate) fn initial_dream_committed_evidence(
        &self,
        pass: &crate::InitialDreamPass,
    ) -> Result<(std::collections::BTreeSet<MemoryId>, bool), CvaError> {
        let access = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        let source = access
            .snapshot()
            .map_err(|error| CvaError::Container(error.0))?;
        let proof = crate::history::dream_evidence::read_dream_evidence(
            &source,
            &self.graph,
            &self.dream_freshness,
            pass,
        )?;
        #[cfg(feature = "freshness-performance-trace")]
        record_dream_evidence_trace(proof.trace);
        Ok((proof.roots, proof.duplicate_origin))
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
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            crate::freshness_storage::FreshnessReplayEntry::Initialize {
                memory_id: id,
                source_turn: destination_turn,
                admitted_at_birth,
            },
            destination_turn,
        )?;
        self.refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn freshness_replay_admit(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            crate::freshness_storage::FreshnessReplayEntry::Admit {
                memory_id: id,
                source_turn: destination_turn,
            },
            destination_turn,
        )?;
        self.refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn freshness_replay_event(
        &mut self,
        event_id: String,
        destination_turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        provenance: Option<crate::freshness_storage::FreshnessEventProvenance>,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            crate::freshness_storage::FreshnessReplayEntry::Event {
                event_id,
                source_turn: destination_turn,
                effects,
                proof,
                provenance,
                semantic: None,
            },
            destination_turn,
        )?;
        self.refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn freshness_replay_accepted_use_receipt(
        &mut self,
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            crate::freshness_storage::FreshnessReplayEntry::AcceptedUseReceipt {
                use_id,
                source_turn: turn,
                sources,
                policy_version,
            },
            turn,
        )?;
        self.refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn freshness_replay_semantic_event(
        &mut self,
        event_id: String,
        evidence: crate::freshness_storage::SemanticEventEvidence,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<(), CvaError> {
        let destination_turn = evidence.accepted_turn;
        let owner_uuid = self.owner_uuid();
        let outcome = crate::owner_lifecycle::freshness_reconciliation::replay_entry(
            &mut self.owner.container,
            &self.owner.archive,
            &self.owner.memories,
            owner_uuid,
            &mut self.owner.freshness,
            crate::freshness_storage::FreshnessReplayEntry::Event {
                event_id,
                source_turn: destination_turn,
                effects,
                proof: FreshnessEventProof {
                    graph_version: 0,
                    community_generation: None,
                    community_graph_version: None,
                },
                provenance: None,
                semantic: Some(evidence),
            },
            destination_turn,
        )?;
        self.refresh_freshness_replay_due(outcome)
    }

    fn refresh_freshness_replay_due(
        &mut self,
        outcome: Option<crate::owner_lifecycle::freshness_reconciliation::RefreshDue>,
    ) -> Result<(), CvaError> {
        if let Some(outcome) = outcome {
            self.runtime_freshness_refresh_due(outcome.memories, outcome.turn)?;
        }
        Ok(())
    }

    pub(crate) fn freshness_publication_birth_ids(&self) -> Vec<MemoryId> {
        self.dream_freshness.publication_birth_ids()
    }

    pub(crate) fn freshness_replay_publication_birth(
        &mut self,
        id: MemoryId,
    ) -> Result<(), CvaError> {
        self.runtime_require_freshness_memory(id)?;
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
