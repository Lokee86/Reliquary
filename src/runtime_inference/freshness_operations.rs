use crate::{CvaError, FreshnessEventCut, FreshnessEventProof, FreshnessRecord, MemoryId};
use std::collections::BTreeMap;

#[cfg(feature = "freshness-performance-trace")]
pub(crate) fn flush_freshness_trace() {
    crate::freshness::flush_operation_trace();
}

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

#[cfg(feature = "freshness-performance-trace")]
pub(crate) fn run_initial_dream_trace_fixture(
    owner: &mut crate::RelExecutionOwner,
    source_id: MemoryId,
    preexisting_roots: &[MemoryId],
    workers: usize,
) -> Result<bool, CvaError> {
    let owner_uuid = owner
        .runtime_owner_uuid()
        .ok_or_else(|| CvaError::Freshness("fixture lacks owner UUID".into()))?;
    let pass_id =
        crate::owner_lifecycle::freshness_operations::initial_dream_pass_id(owner_uuid, source_id);
    if owner.native_initial_dream_pass_settled(&pass_id) {
        return Ok(false);
    }
    let Some(pass) = begin_initial_dream_pass(owner, source_id)? else {
        return Err(CvaError::Freshness(
            "fixture source is not eligible for initial Dream".into(),
        ));
    };
    let mut published = Vec::new();
    for root in preexisting_roots {
        let changes = [crate::GraphRelationChange {
            source: source_id,
            target: *root,
            kind: crate::GraphRelationKind::Topical,
            active: true,
        }];
        let graph_version = owner.runtime_graph_version();
        if let Some(relation) = owner
            .runtime_set_knowledge_relations(
                &changes,
                crate::GraphRelationOrigin::Dream,
                graph_version,
            )
            .map_err(|error| CvaError::Freshness(error.to_string()))?
            .into_iter()
            .next()
        {
            published.push(relation);
        }
    }
    record_initial_dream_publication_roots(
        owner,
        source_id,
        &crate::DreamPublicationOutcome::Published(published),
    )?;
    let turn = owner.runtime_rel_turn_count();
    owner.native_accept_initial_dream_settlement(&pass.pass_id, turn)?;
    let Some(work) = prepare_initial_dream_settlement(owner, source_id, false)? else {
        return Err(CvaError::Freshness(
            "fixture initial Dream did not prepare".into(),
        ));
    };
    let result = work.evaluate(workers)?;
    commit_initial_dream_settlement(owner, result)?;
    Ok(true)
}

impl crate::RelExecutionOwner {
    pub(crate) fn native_freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        crate::freshness_operations_runtime::policy(self)
    }

    pub(crate) fn native_freshness_has_pending_publication(&self, id: MemoryId) -> bool {
        self.runtime_has_pending_publication(id)
    }

    pub(crate) fn native_freshness_is_publication_birth(&self, id: MemoryId) -> bool {
        self.owner.dream_is_publication_birth(id)
    }

    pub(crate) fn native_freshness_has_pending_dream_passes(&self) -> bool {
        self.owner.dream_has_pending()
    }

    pub(crate) fn native_initial_dream_pass_settled(&self, pass_id: &str) -> bool {
        self.owner.dream_is_settled(pass_id)
    }

    pub(crate) fn native_freshness_event_proof_is_available(
        &self,
        proof: FreshnessEventProof,
    ) -> bool {
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

    pub(crate) fn native_set_freshness_policy_before_enrollment(
        &mut self,
        policy: crate::freshness::FreshnessPolicy,
    ) -> Result<(), CvaError> {
        crate::freshness_operations_runtime::set_policy_before_enrollment(self, policy)
    }

    pub(crate) fn native_freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.owner
            .current_freshness_rewrite_record(id, self.owner.archive.rel_turn_count())
    }

    pub(crate) fn native_freshness_record_at_cut(
        &self,
        id: MemoryId,
        cut: &FreshnessEventCut,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        if cut.rel_turn > self.runtime_rel_turn_count()
            || cut.graph_version > self.runtime_memory_graph_version()
            || cut
                .event_id_inclusive
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(CvaError::Freshness(
                "invalid historical Freshness event cut".into(),
            ));
        }
        self.owner
            .freshness_record_at_cut(id, cut)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_freshness_record_at(
        &self,
        id: MemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, CvaError> {
        if turn > self.runtime_rel_turn_count() {
            return Err(CvaError::Freshness(
                "Freshness cut is beyond the REL activity head".into(),
            ));
        }
        self.owner
            .freshness_record_at(id, turn)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_freshness_records_at(
        &self,
        turn: u64,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        if turn > self.runtime_rel_turn_count() {
            return Err(CvaError::Freshness(
                "Freshness cut is beyond the REL activity head".into(),
            ));
        }
        self.owner
            .freshness_records_at(turn)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_enroll_legacy_freshness_baseline(
        &mut self,
        id: MemoryId,
    ) -> Result<bool, CvaError> {
        crate::freshness_operations_runtime::enroll_legacy_baseline(self, id)
    }

    pub(crate) fn native_freshness_initialize(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.runtime_freshness_initialize(id)
    }

    pub(crate) fn native_freshness_admit(
        &mut self,
        id: MemoryId,
        turn: u64,
    ) -> Result<bool, CvaError> {
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)?;
        Ok(changed)
    }

    pub(crate) fn native_validate_initial_dream_event_identity(
        &self,
        event_id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &BTreeMap<MemoryId, i16>,
    ) -> Result<(), CvaError> {
        let owner = self.runtime_owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before events".into())
        })?;
        let pass = self.owner.dream_pending(event_id).ok_or_else(|| {
            CvaError::Freshness("Dream event lacks its accepted initial-pass receipt".into())
        })?;
        if event_id
            != crate::owner_lifecycle::freshness_operations::initial_dream_pass_id(
                owner,
                pass.source_id,
            )
            || pass.accepted_settlement_turn != Some(turn)
            || !self.native_freshness_is_publication_birth(pass.source_id)
            || !self
                .owner
                .dream_accepted_graph_cut(event_id)
                .is_some_and(|(_, accepted)| accepted == proof)
            || pass.roots.is_empty()
            || !pass.roots.is_subset(&pass.preexisting_ids)
        {
            return Err(CvaError::Freshness(
                "Dream event lacks matching source, roots, settlement cut, or pinned proof".into(),
            ));
        }
        let cohort = self.native_freshness_publication_cohort(pass.source_id)?;
        if effects.keys().any(|id| cohort.contains(id)) {
            return Err(CvaError::Freshness(
                "Dream event cannot reinforce its newly created publication cohort".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn native_freshness_commit_event(
        &mut self,
        event_id: impl Into<String>,
        turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<bool, CvaError> {
        let event_id = event_id.into();
        self.native_validate_initial_dream_event_identity(&event_id, turn, proof, &effects)?;
        if self
            .owner
            .freshness_contains_event(&event_id)
            .map_err(CvaError::Freshness)?
        {
            return if self
                .owner
                .freshness_event_matches(&event_id, turn, proof, &effects)
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
        if !self.native_freshness_event_proof_is_available(proof) {
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
        let owner = self.runtime_owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before events".into())
        })?;
        let affected = effects.keys().copied().collect::<Vec<_>>();
        for (id, amount) in &effects {
            self.runtime_require_freshness_memory(*id)?;
            if *amount <= 0
                || *amount > crate::freshness_operations_runtime::policy(self).linkage_principal
            {
                return Err(CvaError::Freshness(
                    "event effect exceeds the pinned positive principal bound".into(),
                ));
            }
            let _record = self.owner.freshness_record(*id).ok_or_else(|| {
                CvaError::Freshness(format!("Memory {:?} has no Freshness enrollment", id))
            })?;
        }
        let postimages = self
            .owner
            .freshness_prepare_event_postimages(&event_id, turn, &effects)
            .map_err(CvaError::Freshness)?;
        self.owner
            .freshness_validate_event_payload(&event_id, turn, proof, &effects, &postimages)
            .map_err(CvaError::Freshness)?;
        let payload = self
            .owner
            .freshness_encode_event(owner, event_id, turn, proof, effects, postimages);
        self.owner
            .freshness_persist_policy_if_needed(owner)
            .map_err(CvaError::Freshness)?;
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .freshness_ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.runtime_freshness_refresh_due(affected, turn)?;
        Ok(true)
    }

    pub(crate) fn native_begin_initial_dream_pass(
        &mut self,
        pass_id: String,
        source_id: MemoryId,
        first_revision: u64,
        turn: u64,
        preexisting_ids: std::collections::BTreeSet<MemoryId>,
        graph_version_before: u64,
    ) -> Result<crate::InitialDreamPass, CvaError> {
        self.runtime_require_freshness_memory(source_id)?;
        let owner = self.runtime_owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before Dream pass".into())
        })?;
        let expected_id =
            crate::owner_lifecycle::freshness_operations::initial_dream_pass_id(owner, source_id);
        if pass_id != expected_id || !self.native_freshness_is_publication_birth(source_id) {
            return Err(CvaError::Freshness(
                "Dream pass lacks canonical first-publication identity".into(),
            ));
        }
        if graph_version_before != self.runtime_graph_version() {
            return Err(CvaError::Freshness(
                "Dream pass graph cut does not match the accepted graph head".into(),
            ));
        }
        let first = self.runtime_memory_revision(source_id, 1)?;
        let current = self.runtime_memory(source_id)?;
        if current.archived
            || !(first.lifecycle_state == "extracted" && current.lifecycle_state == "extracted"
                || matches!(first.lifecycle_state.as_str(), "knowledge" | "canonical"))
        {
            return Err(CvaError::Freshness(
                "Memory is not eligible for its initial Dream pass".into(),
            ));
        }
        for id in &preexisting_ids {
            let candidate = self.runtime_memory_revision(*id, 1)?;
            if !self.native_freshness_is_publication_birth(*id)
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
        let owner = self.runtime_owner_uuid().ok_or_else(|| {
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
            .owner
            .dream_begin_is_retry(&pass)
            .map_err(CvaError::Freshness)?
        {
            return self
                .owner
                .dream_pending_for_source(source_id)
                .ok_or_else(|| CvaError::Freshness("pass receipt missing".into()));
        }
        let payload = self.runtime_current_dream_begin_payload(&pass)?;
        self.owner
            .freshness_persist_policy_if_needed(owner)
            .map_err(CvaError::Freshness)?;
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest_materialized_begin(owner, pass)
            .map_err(CvaError::Freshness)?;
        self.owner
            .dream_pending_for_source(source_id)
            .ok_or_else(|| CvaError::Freshness("pass receipt missing after append".into()))
    }

    pub(crate) fn native_pending_initial_dream_pass(
        &self,
        source_id: MemoryId,
    ) -> Option<crate::InitialDreamPass> {
        self.owner.dream_pending_for_source(source_id)
    }

    pub(crate) fn native_accept_initial_dream_settlement(
        &mut self,
        pass_id: &str,
        turn: u64,
    ) -> Result<bool, CvaError> {
        if turn > self.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "settlement turn exceeds accepted owner REL activity".into(),
            ));
        }
        if let Some(pass) = self.owner.dream_pending(pass_id)
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
        let snapshot = self.runtime_community_snapshot().filter(|snapshot| {
            snapshot.derived_graph_version == self.runtime_memory_graph_version()
        });
        let proof = FreshnessEventProof {
            graph_version: self.runtime_memory_graph_version(),
            community_generation: snapshot.as_ref().map(|snapshot| snapshot.generation),
            community_graph_version: snapshot
                .as_ref()
                .map(|snapshot| snapshot.derived_graph_version),
        };
        let payload = self
            .owner
            .dream_accept_graph_payload(pass_id, turn, self.runtime_graph_version(), proof)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(false);
        }
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn native_record_initial_dream_roots(
        &mut self,
        pass_id: &str,
        roots: std::collections::BTreeSet<MemoryId>,
    ) -> Result<(), CvaError> {
        let payload = self
            .owner
            .dream_roots_payload(pass_id, &roots)
            .map_err(CvaError::Freshness)?;
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_settle_initial_dream_pass(
        &mut self,
        pass_id: &str,
        active_admission: bool,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<bool, CvaError> {
        if self.owner.dream_is_settled(pass_id) {
            return Ok(false);
        }
        let pass = self
            .owner
            .dream_pending(pass_id)
            .ok_or_else(|| CvaError::Freshness("Dream pass is not pending".into()))?;
        let turn = pass.accepted_settlement_turn.ok_or_else(|| {
            CvaError::Freshness("Dream settlement acceptance cut is missing".into())
        })?;
        if !self.native_freshness_event_proof_is_available(proof) {
            return Err(CvaError::Freshness(
                "Dream event graph proof is unavailable".into(),
            ));
        }
        if self
            .owner
            .dream_accepted_graph_cut(pass_id)
            .is_some_and(|(_, accepted)| accepted != proof)
        {
            return Err(CvaError::Freshness(
                "Dream event proof differs from accepted settlement snapshot".into(),
            ));
        }
        if !effects.is_empty() {
            self.native_validate_initial_dream_event_identity(pass_id, turn, proof, &effects)?;
        }
        if !effects.is_empty()
            && !self
                .owner
                .freshness_event_matches(pass_id, turn, proof, &effects)
                .map_err(CvaError::Freshness)?
        {
            if self
                .owner
                .freshness_contains_event(pass_id)
                .map_err(CvaError::Freshness)?
            {
                return Err(CvaError::Freshness(
                    "Dream pass event identity collides with different payload".into(),
                ));
            }
            let postimages = self
                .owner
                .freshness_prepare_event_postimages(pass_id, turn, &effects)
                .map_err(CvaError::Freshness)?;
            self.owner
                .freshness_validate_event_payload(pass_id, turn, proof, &effects, &postimages)
                .map_err(CvaError::Freshness)?;
        }
        if active_admission {
            let _ = self.native_freshness_admit(pass.source_id, turn)?;
        }
        if !effects.is_empty() {
            let _ =
                self.native_freshness_commit_event(pass.pass_id.clone(), turn, proof, effects)?;
        }
        let payload = self
            .owner
            .dream_settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn native_freshness_replay_entries(
        &self,
    ) -> Result<Vec<crate::freshness_storage::FreshnessReplayEntry>, CvaError> {
        self.owner
            .freshness_replay_entries()
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_freshness_publication_cohort(
        &self,
        source: MemoryId,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        let history = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        crate::history::publication_birth_cohort(&history, source)
    }

    pub(crate) fn native_freshness_same_publication_cohort(
        &self,
        a: MemoryId,
        b: MemoryId,
    ) -> Result<bool, CvaError> {
        let history = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        crate::history::same_publication_birth_cohort(&history, a, b)
    }

    pub(crate) fn native_initial_dream_graph_snapshot(
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
            .owner
            .dream_accepted_graph_cut(pass_id)
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
                            crate::freshness_operations_runtime::graph_publication_version(
                                object, payload,
                            )?
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
        crate::freshness::record_initial_dream_graph_snapshot(
            trace_started.elapsed().as_nanos(),
            trace_payload_bytes,
            trace_object_bytes,
            trace_chunks,
            trace_mutations,
            trace_communities,
            trace_community_matches,
            trace_filtered,
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

    pub(crate) fn native_initial_dream_committed_roots(
        &self,
        pass: &crate::InitialDreamPass,
    ) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
        self.native_initial_dream_committed_evidence(pass)
            .map(|(roots, _)| roots)
    }

    pub(crate) fn native_initial_dream_committed_evidence(
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
        crate::freshness::record_dream_evidence_scan(
            proof.trace.elapsed_ns,
            proof.trace.payload_bytes,
            proof.trace.object_bytes,
            proof.trace.chunks,
            proof.trace.mutations,
            proof.trace.filtered,
        );
        Ok((proof.roots, proof.duplicate_origin))
    }

    pub(crate) fn native_recover_committed_initial_dream_event(
        &mut self,
        pass_id: &str,
        active_admission: bool,
    ) -> Result<bool, CvaError> {
        let Some((turn, _, _)) = self
            .owner
            .freshness_event_receipt(pass_id)
            .map_err(CvaError::Freshness)?
        else {
            return Ok(false);
        };
        let pass = self.owner.dream_pending(pass_id).ok_or_else(|| {
            CvaError::Freshness("committed Dream event has no pending pass".into())
        })?;
        if pass.accepted_settlement_turn != Some(turn) {
            return Err(CvaError::Freshness(
                "committed Dream event differs from accepted settlement cut".into(),
            ));
        }
        if active_admission {
            self.native_freshness_admit(pass.source_id, turn)?;
        }
        let payload = self
            .owner
            .dream_settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub(crate) fn native_freshness_replay_initialize(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
        admitted_at_birth: bool,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn native_freshness_replay_admit(
        &mut self,
        id: MemoryId,
        destination_turn: u64,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn native_freshness_replay_event(
        &mut self,
        event_id: String,
        destination_turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        provenance: Option<crate::freshness_storage::FreshnessEventProvenance>,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn native_freshness_replay_accepted_use_receipt(
        &mut self,
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    ) -> Result<(), CvaError> {
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn native_freshness_replay_semantic_event(
        &mut self,
        event_id: String,
        evidence: crate::freshness_storage::SemanticEventEvidence,
        effects: BTreeMap<MemoryId, i16>,
    ) -> Result<(), CvaError> {
        let destination_turn = evidence.accepted_turn;
        let owner_uuid = self.runtime_owner_uuid();
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
        self.native_refresh_freshness_replay_due(outcome)
    }

    pub(crate) fn native_refresh_freshness_replay_due(
        &mut self,
        outcome: Option<crate::owner_lifecycle::freshness_reconciliation::RefreshDue>,
    ) -> Result<(), CvaError> {
        if let Some(outcome) = outcome {
            self.runtime_freshness_refresh_due(outcome.memories, outcome.turn)?;
        }
        Ok(())
    }

    pub(crate) fn native_freshness_publication_birth_ids(&self) -> Vec<MemoryId> {
        self.owner.dream_publication_birth_ids()
    }

    pub(crate) fn native_freshness_replay_publication_birth(
        &mut self,
        id: MemoryId,
    ) -> Result<(), CvaError> {
        self.runtime_require_freshness_memory(id)?;
        let payload = self
            .owner
            .dream_replay_publication_birth_payload(id)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(());
        }
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)
    }

    pub(crate) fn native_freshness_settled_dream_pass_ids(&self) -> Vec<String> {
        self.owner.dream_settled_ids().map(str::to_owned).collect()
    }

    pub(crate) fn native_freshness_replay_settled_dream_pass(
        &mut self,
        pass_id: &str,
    ) -> Result<(), CvaError> {
        let payload = self
            .owner
            .dream_replay_settled_payload(pass_id)
            .map_err(CvaError::Freshness)?;
        if payload.is_empty() {
            return Ok(());
        }
        crate::freshness_operations_runtime::append_and_sync(self, &payload)?;
        self.owner
            .dream_ingest(&payload)
            .map_err(CvaError::Freshness)
    }
}

pub(crate) fn begin_initial_dream_pass(
    owner: &mut crate::RelExecutionOwner,
    source_id: MemoryId,
) -> Result<Option<crate::InitialDreamPass>, CvaError> {
    if let Some(pending) = owner.native_pending_initial_dream_pass(source_id) {
        return Ok(Some(pending));
    }
    let source = owner.runtime_memory(source_id)?;
    let first = owner.runtime_memory_revision(source_id, 1)?;
    let Some(owner_uuid) = owner_uuid(owner) else {
        return Ok(None);
    };
    let pass_id =
        crate::owner_lifecycle::freshness_operations::initial_dream_pass_id(owner_uuid, source_id);
    if owner.owner.dream_is_settled(&pass_id) {
        return Ok(None);
    }
    let has_publication_birth = owner.owner.dream_is_publication_birth(source_id);
    let first_extracted_settlement = has_publication_birth
        && first.lifecycle_state == "extracted"
        && source.lifecycle_state == "extracted"
        && !source.archived;
    let direct_active_birth = has_publication_birth
        && !source.archived
        && matches!(first.lifecycle_state.as_str(), "knowledge" | "canonical");
    if !first_extracted_settlement && !direct_active_birth {
        return Ok(None);
    }
    // Ranking inputs cannot narrow durable first-pass authority. Capture the
    // accepted physical birth/cohort prefix used by compact Begin replay.
    let prefix_end = owner.owner.container.physical_length()?;
    let preexisting_ids = owner.native_dream_population_at_prefix(
        prefix_end,
        owner.native_memory_version(),
        source_id,
    )?;
    owner
        .native_begin_initial_dream_pass(
            pass_id,
            source_id,
            1,
            owner.runtime_rel_turn_count(),
            preexisting_ids,
            owner.runtime_graph_version(),
        )
        .map(Some)
}

pub(crate) fn record_initial_dream_publication_roots(
    owner: &mut crate::RelExecutionOwner,
    source_id: MemoryId,
    publication: &crate::DreamPublicationOutcome,
) -> Result<(), CvaError> {
    let Some(pass) = owner.native_pending_initial_dream_pass(source_id) else {
        return Ok(());
    };
    let mut roots = roots_from_current_graph(owner, source_id, &pass);
    if let crate::DreamPublicationOutcome::Published(relations) = publication {
        for relation in relations {
            if relation.origin != crate::GraphRelationOrigin::Dream {
                continue;
            }
            let neighbor = if relation.source == source_id {
                Some(relation.target)
            } else if relation.target == source_id {
                Some(relation.source)
            } else {
                None
            };
            if let Some(neighbor) = neighbor
                && pass.preexisting_ids.contains(&neighbor)
            {
                roots.insert(neighbor);
            }
        }
    }
    owner.native_record_initial_dream_roots(&pass.pass_id, roots)
}

fn roots_from_current_graph(
    owner: &crate::RelExecutionOwner,
    source_id: MemoryId,
    pass: &crate::InitialDreamPass,
) -> std::collections::BTreeSet<MemoryId> {
    owner
        .runtime_graph_relations()
        .into_iter()
        .filter(|relation| {
            relation.origin == crate::GraphRelationOrigin::Dream
                && relation.active
                && relation.graph_version > pass.graph_version_before
                && (relation.source == source_id || relation.target == source_id)
        })
        .filter_map(|relation| {
            let neighbor = if relation.source == source_id {
                relation.target
            } else {
                relation.source
            };
            pass.preexisting_ids.contains(&neighbor).then_some(neighbor)
        })
        .collect()
}

pub(crate) fn policy(owner: &crate::RelExecutionOwner) -> crate::freshness::FreshnessPolicy {
    owner.owner.freshness_policy()
}

pub(crate) fn set_policy_before_enrollment(
    owner: &mut crate::RelExecutionOwner,
    policy: crate::freshness::FreshnessPolicy,
) -> Result<(), CvaError> {
    crate::owner_lifecycle::freshness_operations::require_write_admission(&owner.owner.container)?;
    if owner.owner.dream_has_any() || owner.owner.freshness_has_any_record() {
        return Err(CvaError::Freshness(
            "Freshness policy is immutable after first enrollment".into(),
        ));
    }
    let owner_uuid =
        crate::owner_lifecycle::freshness_operations::owner_uuid(&owner.owner.container)
            .ok_or_else(|| {
                CvaError::Freshness(
                    "legacy owner requires explicit migration before Freshness policy".into(),
                )
            })?;
    crate::freshness::accepted_use::set_policy_before_enrollment(
        &mut owner.owner.freshness,
        policy,
    )
    .map_err(CvaError::Freshness)?;
    crate::owner_lifecycle::freshness_operations::persist_policy_if_needed(
        &mut owner.owner,
        owner_uuid,
    )
    .map_err(CvaError::Freshness)
}

pub(crate) fn enroll_legacy_baseline(
    owner: &mut crate::RelExecutionOwner,
    id: MemoryId,
) -> Result<bool, CvaError> {
    owner.runtime_require_freshness_memory(id)?;
    if owner.owner.dream_is_publication_birth(id) || owner.runtime_has_pending_publication(id) {
        return Err(CvaError::Freshness(
            "a proven first publication cannot be re-enrolled as a legacy baseline".into(),
        ));
    }
    if owner.owner.freshness_has_record(id) {
        return Ok(false);
    }
    let memory = memory(owner, id)?;
    if memory.archived || !matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical") {
        return Err(CvaError::Freshness(
            "legacy baseline requires a nonarchived settled Memory".into(),
        ));
    }
    let owner_uuid = owner_uuid(owner).ok_or_else(|| {
        CvaError::Freshness("legacy owner requires explicit owner migration first".into())
    })?;
    let turn = owner.owner.archive.rel_turn_count();
    let mut record = crate::owner_lifecycle::freshness_operations::created_record();
    crate::owner_lifecycle::freshness_operations::admit_record(&mut record, turn)
        .map_err(|error| CvaError::Freshness(error.to_string()))?;
    crate::owner_lifecycle::freshness_operations::persist_policy_if_needed(
        &mut owner.owner,
        owner_uuid,
    )
    .map_err(CvaError::Freshness)?;
    let payload = crate::freshness::accepted_use::initialize_payload(
        &owner.owner.freshness,
        owner_uuid,
        id,
        turn,
        record,
    );
    append_and_sync(owner, &payload)?;
    crate::freshness::accepted_use::ingest_appended(&mut owner.owner.freshness, &payload)
        .map_err(CvaError::Freshness)?;
    owner.runtime_freshness_refresh_due([id], turn)?;
    Ok(true)
}

pub(crate) fn ego_memory_grades(
    owner: &crate::RelExecutionOwner,
    memory_ids: &[MemoryId],
    as_of_turn: u64,
) -> Result<Vec<(MemoryId, i16, crate::FreshnessState)>, CvaError> {
    crate::freshness::accepted_use::ego_grades(
        &owner.owner.freshness,
        memory_ids,
        as_of_turn,
        owner.owner.archive.rel_turn_count(),
    )
    .map_err(CvaError::Freshness)
}

pub(crate) fn select_ego_routine_context(
    owner: &crate::RelExecutionOwner,
    candidates: &[(MemoryId, usize)],
    budget: usize,
    hard_keep: &std::collections::BTreeSet<MemoryId>,
    as_of_turn: u64,
) -> Result<Vec<MemoryId>, CvaError> {
    if as_of_turn > owner.owner.archive.rel_turn_count() {
        return Err(CvaError::Freshness(
            "Ego Freshness cut is beyond the REL activity head".into(),
        ));
    }
    let mut unique = std::collections::BTreeMap::<MemoryId, usize>::new();
    for (memory, cost) in candidates {
        unique
            .entry(*memory)
            .and_modify(|existing| *existing = (*existing).min(*cost))
            .or_insert(*cost);
    }
    let total = unique
        .values()
        .try_fold(0usize, |sum, cost| sum.checked_add(*cost));
    if total.is_some_and(|total| total <= budget) {
        let mut all = hard_keep.clone();
        all.extend(unique.keys().copied());
        return Ok(all.into_iter().collect());
    }
    let mut selected = hard_keep.clone();
    let mut ordered = hard_keep.iter().copied().collect::<Vec<_>>();
    let spent = selected
        .iter()
        .filter_map(|id| unique.get(id))
        .try_fold(0usize, |sum, cost| sum.checked_add(*cost))
        .unwrap_or(usize::MAX);
    let graded = ego_memory_grades(
        owner,
        &unique.keys().copied().collect::<Vec<_>>(),
        as_of_turn,
    )?;
    let grades = graded
        .into_iter()
        .map(|(id, score, _state)| (id, score))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut ranked = unique.keys().copied().collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        grades
            .get(b)
            .copied()
            .unwrap_or(0)
            .cmp(&grades.get(a).copied().unwrap_or(0))
            .then_with(|| a.cmp(b))
    });
    let mut remaining = budget.saturating_sub(spent);
    for memory_id in ranked {
        if selected.contains(&memory_id) {
            continue;
        }
        let cost = unique[&memory_id];
        if cost <= remaining {
            selected.insert(memory_id);
            ordered.push(memory_id);
            remaining -= cost;
        }
    }
    Ok(ordered)
}

pub(crate) fn require_write_admission(owner: &crate::RelExecutionOwner) -> Result<(), CvaError> {
    crate::owner_lifecycle::freshness_operations::require_write_admission(&owner.owner.container)?;
    Ok(())
}

pub(crate) fn owner_uuid(owner: &crate::RelExecutionOwner) -> Option<[u8; 16]> {
    crate::owner_lifecycle::freshness_operations::owner_uuid(&owner.owner.container)
}

pub(crate) fn memory(
    owner: &crate::RelExecutionOwner,
    id: MemoryId,
) -> Result<crate::Memory, CvaError> {
    Ok(crate::owner_lifecycle::freshness_operations::read_memory(
        &owner.owner.memories,
        &owner.owner.container,
        id,
    )?)
}

pub(crate) fn append_and_sync(
    owner: &mut crate::RelExecutionOwner,
    payload: &[u8],
) -> Result<(), CvaError> {
    crate::owner_lifecycle::freshness_operations::append_and_sync(
        &mut owner.owner.container,
        payload,
    )?;
    Ok(())
}

pub(crate) fn graph_publication_version(
    object: crate::ObjectRef,
    payload: &[u8],
) -> Result<Option<crate::graph_codec::GraphVersionPayload>, CvaError> {
    Ok(crate::freshness::graph_snapshot::publication_version(
        object, payload,
    )?)
}

pub(crate) fn record_accepted_memory_use(
    owner: &mut crate::RelExecutionOwner,
    owner_uuid: [u8; 16],
    use_id: String,
    accepted_turn: u64,
    requested_memories: Vec<MemoryId>,
    workers: usize,
) -> Result<usize, CvaError> {
    let _trace =
        crate::freshness::operation_trace_scope(crate::freshness::TraceOperation::AcceptedUse);
    {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseReceiptValidation,
        );
        if use_id.trim().is_empty() {
            return Err(CvaError::Freshness(
                "accepted-use receipt has an empty identity".into(),
            ));
        }
        if crate::owner_lifecycle::freshness_operations::owner_uuid(&owner.owner.container)
            != Some(owner_uuid)
        {
            return Err(CvaError::Freshness(
                "accepted-use receipt belongs to a different REL owner".into(),
            ));
        }
        if accepted_turn > owner.owner.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "accepted-use receipt refers to a future REL turn".into(),
            ));
        }
    }

    let memories = {
        let _stage =
            crate::freshness::trace_stage(crate::freshness::TraceStage::AcceptedUseRootCollection);
        let unique: std::collections::BTreeSet<_> = requested_memories.into_iter().collect();
        unique.into_iter().collect::<Vec<_>>()
    };
    let exact_retry = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseExactRetryLookup,
        );
        crate::freshness::accepted_use::retry_exists(
            &owner.owner.freshness,
            &use_id,
            accepted_turn,
            &memories,
        )
        .map_err(CvaError::Freshness)?
    };
    if exact_retry {
        return Ok(0);
    }
    if memories.is_empty() {
        return Ok(0);
    }

    let relations = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseDirectGraphMaterialization,
        );
        owner.runtime_graph_relations()
    };
    let adjacency = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseAdjacencyConstruction,
        );
        let mut adjacency =
            std::collections::BTreeMap::<MemoryId, std::collections::BTreeSet<MemoryId>>::new();
        for relation in relations.into_iter().filter(|relation| relation.active) {
            adjacency
                .entry(relation.source)
                .or_default()
                .insert(relation.target);
            adjacency
                .entry(relation.target)
                .or_default()
                .insert(relation.source);
        }
        adjacency
    };
    let (_graph_version, snapshot, graph_proof) = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseCommunityProofPinning,
        );
        crate::freshness::accepted_use::graph_proof(&owner.owner.graph, &owner.owner.communities)
    };
    let policy = crate::freshness::accepted_use::policy(&owner.owner.freshness);
    let memberships = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseCommunityMembershipMaterialization,
        );
        snapshot
            .map(|snapshot| {
                snapshot
                    .communities
                    .into_iter()
                    .flat_map(|community| {
                        community
                            .members
                            .into_iter()
                            .map(move |memory| (memory, community.id))
                    })
                    .collect::<std::collections::BTreeMap<MemoryId, crate::CommunityId>>()
            })
            .unwrap_or_default()
    };
    for memory_id in &memories {
        if crate::freshness::accepted_use::current_record(
            &owner.owner.freshness,
            *memory_id,
            owner.owner.archive.rel_turn_count(),
        )
        .is_none()
        {
            return Err(CvaError::Freshness(format!(
                "Memory {:?} has no Freshness record",
                memory_id
            )));
        }
    }
    let events = memories
        .iter()
        .map(|memory_id| crate::PropagationEvent {
            roots: vec![*memory_id],
            principal: policy.access_principal,
            excluded: std::collections::BTreeSet::new(),
        })
        .collect::<Vec<_>>();
    let outcomes = {
        let _stage =
            crate::freshness::trace_stage(crate::freshness::TraceStage::AcceptedUsePropagation);
        crate::freshness::accepted_use::propagation(
            &events,
            &adjacency,
            &memberships,
            workers.max(1),
            &policy,
        )
    }
    .map_err(|error| CvaError::Freshness(error.to_string()))?;
    if !crate::freshness::accepted_use::proof_available(
        &owner.owner.graph,
        &owner.owner.communities,
        graph_proof,
    ) {
        return Err(CvaError::Freshness(
            "accepted-use snapshot proof is unavailable".into(),
        ));
    }
    let affected = outcomes
        .iter()
        .flat_map(|candidates| candidates.keys().copied())
        .collect::<std::collections::BTreeSet<_>>();
    let applied = memories.len();
    let payload = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseEffectPostimagePreparation,
        );
        crate::freshness::accepted_use::prepare_payload(
            &owner.owner.freshness,
            owner_uuid,
            use_id,
            accepted_turn,
            memories,
            graph_proof,
            outcomes,
        )
    }
    .map_err(CvaError::Freshness)?;
    crate::owner_lifecycle::freshness_operations::persist_policy_if_needed(
        &mut owner.owner,
        owner_uuid,
    )
    .map_err(CvaError::Freshness)?;
    {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::AcceptedUseContainerAppendSync,
        );
        crate::owner_lifecycle::freshness_operations::append_and_sync(
            &mut owner.owner.container,
            &payload,
        )?;
    }
    crate::freshness::accepted_use::ingest_appended(&mut owner.owner.freshness, &payload)
        .map_err(CvaError::Freshness)?;
    owner.runtime_freshness_refresh_due(affected, accepted_turn)?;
    Ok(applied)
}

pub(crate) fn commit_initial_dream_settlement(
    owner: &mut crate::RelExecutionOwner,
    settlement: crate::dream_settlement::InitialSettlementResult,
) -> Result<(), CvaError> {
    let _trace = crate::freshness::operation_trace_scope(
        crate::freshness::TraceOperation::InitialDreamCommit,
    );
    // Revalidate durable acceptance, the original pinned proof, and the
    // idempotent receipt after the runtime lock has been reacquired.
    let _stage = crate::freshness::trace_stage(
        crate::freshness::TraceStage::DreamReceiptValidationPostimageAppendSyncDueRefresh,
    );
    owner
        .native_settle_initial_dream_pass(
            &settlement.pass_id,
            settlement.active_admission,
            settlement.proof,
            settlement.effects,
        )
        .map(|_| ())
}

pub(crate) fn prepare_initial_dream_settlement(
    owner: &mut crate::RelExecutionOwner,
    source_id: MemoryId,
    active_admission: bool,
) -> Result<Option<crate::dream_settlement::InitialSettlementWork>, CvaError> {
    use std::collections::BTreeSet;

    let _trace = crate::freshness::operation_trace_scope(
        crate::freshness::TraceOperation::InitialDreamPrepare,
    );
    let Some(pass) = owner.native_pending_initial_dream_pass(source_id) else {
        return Ok(None);
    };
    if owner.native_recover_committed_initial_dream_event(&pass.pass_id, active_admission)? {
        return Ok(None);
    }
    let raw_roots = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::DreamDurableHistoricalGraphScan,
        );
        let committed = owner.native_initial_dream_committed_roots(&pass)?;
        pass.roots
            .iter()
            .copied()
            .chain(committed)
            .collect::<BTreeSet<_>>()
    };
    {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::DreamRootCollectionPersistence,
        );
        owner.native_record_initial_dream_roots(&pass.pass_id, raw_roots.clone())?;
    }
    let roots = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::DreamDuplicateRootResolution,
        );
        fold_duplicate_roots(owner, raw_roots, source_id)?
    };
    let (proof, adjacency, memberships) = {
        let _stage = crate::freshness::trace_stage(
            crate::freshness::TraceStage::DreamHistoricalGraphSnapshot,
        );
        owner.native_initial_dream_graph_snapshot(&pass.pass_id)?
    };
    let policy = owner.native_freshness_policy();
    let event = if roots.is_empty() {
        None
    } else {
        Some(crate::PropagationEvent {
            roots: roots.into_iter().collect(),
            principal: policy.linkage_principal,
            excluded: owner.native_freshness_publication_cohort(source_id)?,
        })
    };
    Ok(Some(crate::dream_settlement::InitialSettlementWork {
        pass_id: pass.pass_id,
        active_admission,
        proof,
        event,
        adjacency,
        memberships,
        policy,
    }))
}

pub(crate) fn settle_initial_dream_pass(
    owner: &mut crate::RelExecutionOwner,
    source_id: MemoryId,
    active_admission: bool,
    workers: usize,
) -> Result<(), CvaError> {
    if let Some(work) = prepare_initial_dream_settlement(owner, source_id, active_admission)? {
        commit_initial_dream_settlement(owner, work.evaluate(workers)?)?;
    }
    Ok(())
}

fn fold_duplicate_roots(
    owner: &crate::RelExecutionOwner,
    roots: std::collections::BTreeSet<MemoryId>,
    source_id: MemoryId,
) -> Result<std::collections::BTreeSet<MemoryId>, CvaError> {
    let relations = owner.runtime_graph_relations();
    let parts =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let fact_ids: std::collections::BTreeSet<_> = roots
        .iter()
        .filter(|root| {
            relations.iter().any(|relation| {
                relation.kind == crate::GraphRelationKind::DuplicateOf
                    && (relation.source == **root || relation.target == **root)
            })
        })
        .flat_map(|root| crate::graph_relation_sets::duplicate_component(*root, &relations))
        .collect();
    let facts =
        crate::public_dream_operations::native_dream_owner_facts(owner, fact_ids, Some(source_id));
    let first = parts
        .memory_revision(source_id, 1)
        .map_err(CvaError::from)?;
    let mut result = std::collections::BTreeSet::new();
    for root in roots {
        let is_duplicate = relations.iter().any(|relation| {
            relation.kind == crate::GraphRelationKind::DuplicateOf
                && (relation.source == root || relation.target == root)
        });
        if !is_duplicate {
            result.insert(root);
            continue;
        }
        let representative = crate::dream_canonical::existing_duplicate_representative(
            &parts,
            &facts,
            root,
            first.global_version,
            source_id,
            &relations,
        )
        .map_err(|error| CvaError::Freshness(error.to_string()))?;
        if let Some(representative) = representative {
            result.insert(representative);
        } else if !parts.memory(root).map_err(CvaError::from)?.archived {
            result.insert(root);
        }
    }
    Ok(result)
}
