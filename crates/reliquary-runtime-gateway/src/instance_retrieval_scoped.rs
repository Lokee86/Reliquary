use super::*;
use crate::instance_observation_reads::{
    ObservationRead, ObservationRetrievalCandidate, observation_candidates,
};
use crate::instance_retrieval::preview::project_memories;
use reliquary_application::{AgenticScope, RetrievalConfig, evaluate_scoped_snapshot_retrieval};
use reliquary_owner_session::{
    PhyAuditCut, RelRetrievalMemory, RelRetrievalSnapshot,
    model::{CompatibilityProfile, MemoryBodyId, MemoryRevision, NodeRef, PayloadState},
};
use std::collections::BTreeMap;
use std::sync::{Weak, atomic::AtomicBool};

macro_rules! read_phy {
    ($read:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $read {
            ObservationRead::Phy(owner) => owner.$method($($arg),*),
            ObservationRead::PhyWriter(owner) => owner.$method($($arg),*),
            ObservationRead::Rel(_) => unreachable!("PHY read bound by Gateway"),
        }
    };
}

/// The trusted host's active PHY for a live profile. All clones are fenced
/// together before the host replaces the profile's private owner binding.
#[derive(Clone)]
pub struct ActivePhyRecall {
    owner: OwnerId,
    active: Arc<AtomicBool>,
}

impl ActivePhyRecall {
    pub fn new(owner: OwnerId) -> Self {
        Self {
            owner,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn owner(&self) -> OwnerId {
        self.owner
    }

    pub fn invalidate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

enum PhyMount {
    Read(Weak<PhySession>),
    Write(Weak<MountedPhyWriter>),
}

impl PhyMount {
    fn capture(instance: &Instance, owner: OwnerId) -> Result<Self, GatewayError> {
        let mounted = instance
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match mounted.get(&owner) {
            Some(MountedOwner::Phy(phy)) => Ok(Self::Read(Arc::downgrade(phy))),
            Some(MountedOwner::PhyWriter(phy)) => Ok(Self::Write(Arc::downgrade(phy))),
            Some(_) => Err(GatewayError::WrongOwnerKind(owner)),
            None => Err(GatewayError::UnknownOwner(owner)),
        }
    }
}

pub(super) struct PhyRecallBinding {
    active: ActivePhyRecall,
    mount: PhyMount,
    history_generation: [u8; 32],
}

impl PhyRecallBinding {
    pub(super) fn owner(&self) -> OwnerId {
        self.active.owner()
    }

    pub(super) fn history_generation(&self) -> [u8; 32] {
        self.history_generation
    }

    pub(super) fn validate(&self, instance: &Instance) -> Result<(), GatewayError> {
        instance.authorize_ego_phy(self.owner())?;
        if !self.active.active.load(Ordering::Acquire) {
            return Err(GatewayError::StaleRetrieval);
        }
        let current = PhyMount::capture(instance, self.owner()).map_err(|error| match error {
            GatewayError::UnknownOwner(_) | GatewayError::WrongOwnerKind(_) => {
                GatewayError::StaleRetrieval
            }
            other => other,
        })?;
        let same = match (&self.mount, current) {
            (PhyMount::Read(prior), PhyMount::Read(current)) => prior.ptr_eq(&current),
            (PhyMount::Write(prior), PhyMount::Write(current)) => prior.ptr_eq(&current),
            _ => false,
        };
        same.then_some(()).ok_or(GatewayError::StaleRetrieval)
    }
}

impl Instance {
    pub fn begin_scoped_memory_retrieval(
        &self,
        rel: Option<OwnerId>,
        active: ActivePhyRecall,
        turn_id: &str,
        mode: RetrievalMode,
    ) -> Result<MemoryRetrievalTurn, GatewayError> {
        let owner = active.owner();
        self.authorize_ego_phy(owner)?;
        let mount = PhyMount::capture(self, owner)?;
        let history_generation = self.with_observation_read(owner, true, |read| {
            read_phy!(read, phy_audit_cut()).history_generation
        })?;
        let phy = PhyRecallBinding {
            active,
            mount,
            history_generation,
        };
        self.begin_bound_memory_retrieval(rel, Some(phy), turn_id, mode)
    }

    /// Recheck the scoped mount and credential before any external use.
    pub fn validate_memory_retrieval(
        &self,
        turn: &MemoryRetrievalTurn,
    ) -> Result<(), GatewayError> {
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)
    }

    pub fn phy_retrieval_embedding_profiles(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<CompatibilityProfile>, GatewayError> {
        self.with_observation_read(owner, true, |read| {
            let mut profiles = read_phy!(read, memory_vector_infos())
                .into_iter()
                .map(|info| info.compatibility_profile_id)
                .filter(|profile| read_phy!(read, active_compatibility_profile(*profile)).is_some())
                .collect::<BTreeSet<_>>();
            match read {
                ObservationRead::Phy(owner) => profiles.extend(owner.observation_claim_profiles()),
                ObservationRead::PhyWriter(owner) => {
                    profiles.extend(owner.observation_claim_profiles())
                }
                ObservationRead::Rel(_) => unreachable!("PHY read bound by Gateway"),
            }
            profiles
                .into_iter()
                .filter_map(|profile| {
                    read_phy!(read, active_compatibility_profile(profile)).cloned()
                })
                .collect()
        })
    }

    pub(super) fn capture_scoped_recall(
        &self,
        turn: &MemoryRetrievalTurn,
        profiles: (
            Option<CompatibilityProfileId>,
            Option<CompatibilityProfileId>,
        ),
        include_vectors: bool,
        excluded: &BTreeSet<ObservationCitation>,
        traverse: bool,
    ) -> Result<ScopedRecall, GatewayError> {
        let binding = turn
            .phy
            .as_ref()
            .ok_or(GatewayError::InvalidPreparedRetrieval)?;
        self.validate_memory_retrieval(turn)?;
        let rel = if turn.mount.is_some() {
            Some(self.with_reliquary(turn.owner, |rel| {
                let profile = profiles
                    .0
                    .filter(|id| rel.active_compatibility_profile(*id).is_some());
                Ok::<_, GatewayError>(OwnerRecall {
                    snapshot: capture_memory_snapshot(rel, profile, include_vectors)?,
                    profile,
                    publication: ObservationReadCut::capture(rel).publication,
                    removal_cut: rel.archive_source_removal_version(),
                    observations: if traverse {
                        BTreeMap::new()
                    } else {
                        observation_retrieval_candidates(
                            rel,
                            profile.filter(|_| include_vectors),
                            excluded,
                        )?
                    },
                })
            })??)
        } else {
            None
        };
        let (phy_cut, phy) = self.with_observation_read(binding.owner(), true, |read| {
            let phy_cut = read_phy!(read, phy_audit_cut());
            if phy_cut.history_generation != binding.history_generation {
                return Err(GatewayError::StaleRetrieval);
            }
            let profile = profiles
                .1
                .filter(|id| read_phy!(read, active_compatibility_profile(*id)).is_some());
            let mut snapshot = RelRetrievalSnapshot {
                owner_id: phy_cut.owner_id,
                rel_turn: 0,
                memory_version: phy_cut.memory,
                graph_version: phy_cut.graph,
                memories: Vec::new(),
                active_relations: read_phy!(read, graph_relations()),
                community: None,
                vectors: BTreeMap::new(),
            };
            let mut after = None;
            loop {
                let page = read_phy!(read, phy_audit_initial_page(phy_cut, after))?;
                for id in page.sources {
                    let Some((record, body)) = read_phy!(read, phy_audit_evidence(phy_cut, id))?
                    else {
                        continue;
                    };
                    if let Some(profile) = profile.filter(|_| include_vectors) {
                        if let Some(vector) =
                            read_phy!(read, memory_vector_row(profile, record.body_id))?
                        {
                            snapshot.vectors.insert(record.body_id, vector);
                        }
                    }
                    snapshot.memories.push(RelRetrievalMemory {
                        revision: MemoryRevision {
                            record,
                            payload: PayloadState::Available(body),
                        },
                        freshness: None,
                    });
                }
                after = page.next;
                if after.is_none() {
                    break;
                }
            }
            Ok((
                phy_cut,
                OwnerRecall {
                    snapshot,
                    profile,
                    publication: read_phy!(read, perception_review_version()),
                    removal_cut: 0,
                    observations: if traverse {
                        BTreeMap::new()
                    } else {
                        observation_candidates(read, profile.filter(|_| include_vectors), excluded)?
                    },
                },
            ))
        })??;
        self.validate_memory_retrieval(turn)?;
        Ok(ScopedRecall { rel, phy, phy_cut })
    }

    pub(super) fn check_phy_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        generation: Option<[u8; 32]>,
        graph: Option<u64>,
        memories: &[RetrievalMemoryReference],
        observations: impl IntoIterator<Item = ObservationCitation>,
    ) -> Result<(), GatewayError> {
        let Some(binding) = &turn.phy else {
            return if generation.is_none() {
                Ok(())
            } else {
                Err(GatewayError::InvalidPreparedRetrieval)
            };
        };
        let owner = binding.owner();
        if generation != Some(binding.history_generation) {
            return Err(GatewayError::StaleRetrieval);
        }
        self.with_observation_read(owner, true, |read| {
            let cut = read_phy!(read, phy_audit_cut());
            if cut.history_generation != binding.history_generation
                || graph.is_some_and(|graph| cut.graph != graph)
            {
                return Err(GatewayError::StaleRetrieval);
            }
            for memory in memories {
                let id = parse_memory_id(&memory.memory_id)?;
                let exact =
                    read_phy!(read, phy_audit_evidence(cut, id))?.is_some_and(|(record, _)| {
                        memory.owner_id == hex(&owner.as_bytes())
                            && memory.revision == record.revision
                            && memory.body_id == hex(&record.body_id.0)
                    });
                if !exact {
                    return Err(GatewayError::StaleRetrieval);
                }
            }
            for citation in observations
                .into_iter()
                .filter(|cite| cite.owner_id == owner.as_bytes())
            {
                if read.citation_text(&citation).is_err() {
                    return Err(GatewayError::StaleRetrieval);
                }
            }
            Ok(())
        })??;
        self.validate_memory_retrieval(turn)
    }
    pub(super) fn revalidate_recovered_phy<'a>(
        &self,
        turn: &MemoryRetrievalTurn,
        manifest: &DeliveryManifest,
        memories: impl IntoIterator<Item = &'a ManifestMemory>,
    ) -> Result<(), GatewayError> {
        let binding = turn
            .phy
            .as_ref()
            .ok_or(GatewayError::InvalidPreparedRetrieval)?;
        self.with_observation_read(binding.owner(), true, |read| {
            let cut = read_phy!(read, phy_audit_cut());
            if Some(cut.history_generation) != manifest.phy_generation
                || manifest.phy_graph.is_some_and(|graph| cut.graph != graph)
            {
                return Err(GatewayError::StaleRetrieval);
            }
            for memory in memories {
                let id = parse_memory_id(&memory.memory_id)?;
                let exact =
                    read_phy!(read, phy_audit_evidence(cut, id))?.is_some_and(|(record, body)| {
                        record.revision == memory.revision
                            && hex(&record.body_id.0) == memory.body_id
                            && body.title == memory.title
                            && body.content == memory.content
                    });
                if !exact {
                    return Err(GatewayError::StaleRetrieval);
                }
            }
            for claim in manifest
                .observations
                .iter()
                .filter(|claim| claim.citation.owner_id == cut.owner_id)
            {
                let ObservationSource::Observation(id) = claim.citation.source else {
                    return Err(GatewayError::InvalidHandoffReceipt);
                };
                if read.citation_text(&claim.citation).is_err()
                    || !read_phy!(read, observation_revision(id, claim.citation.revision))
                        .is_some_and(|row| ObservationClaim::from(row.revision) == *claim)
                {
                    return Err(GatewayError::StaleRetrieval);
                }
            }
            Ok(())
        })??;
        self.validate_memory_retrieval(turn)
    }

    pub fn validate_inspected_scoped_selection(
        &self,
        turn: &MemoryRetrievalTurn,
        memories: &[RetrievalMemoryReference],
        observations: &[ObservationCitation],
    ) -> Result<Vec<MemoryId>, GatewayError> {
        self.validate_memory_retrieval(turn)?;
        let binding = turn
            .phy
            .as_ref()
            .ok_or(GatewayError::InvalidPreparedRetrieval)?;
        let phy_owner = hex(&binding.owner().as_bytes());
        let rel_owner = hex(&turn.owner.as_bytes());
        let (phy, rel): (Vec<_>, Vec<_>) = memories
            .iter()
            .cloned()
            .partition(|memory| memory.owner_id == phy_owner);
        if rel.iter().any(|memory| memory.owner_id != rel_owner)
            || observations.iter().any(|cite| {
                cite.owner_id != binding.owner().as_bytes()
                    && (turn.mount.is_none() || cite.owner_id != turn.owner.as_bytes())
            })
        {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        self.check_phy_delivery(
            turn,
            Some(binding.history_generation),
            None,
            &phy,
            observations.iter().copied(),
        )?;
        if turn.mount.is_some() {
            self.with_reliquary(turn.owner, |owner| {
                let snapshot = capture_memory_snapshot(owner, None, false)?;
                let rel_current = rel.iter().all(|memory| {
                    snapshot.memories.iter().any(|row| {
                        let record = &row.revision.record;
                        memory.memory_id == hex(&record.id.0)
                            && memory.revision == record.revision
                            && memory.body_id == hex(&record.body_id.0)
                    })
                });
                let observations_current = observations
                    .iter()
                    .filter(|cite| cite.owner_id == turn.owner.as_bytes())
                    .all(|cite| owner.observation_citation_text(cite).is_ok());
                if !rel_current || !observations_current {
                    return Err(GatewayError::StaleRetrieval);
                }
                Ok(())
            })??;
        }
        self.validate_memory_retrieval(turn)?;
        memories
            .iter()
            .map(|memory| {
                let id = parse_memory_id(&memory.memory_id)?;
                Ok(if memory.owner_id == phy_owner {
                    phy_memory_key(binding.owner().as_bytes(), id)
                } else {
                    id
                })
            })
            .collect()
    }

    /// Confirm the exact owner-qualified IDs disclosed by an inspecting host.
    pub fn accept_inspected_scoped_selection(
        &self,
        turn: &MemoryRetrievalTurn,
        memories: &[RetrievalMemoryReference],
        observations: &[ObservationCitation],
    ) -> Result<(), GatewayError> {
        if turn.mode != RetrievalMode::Inspect
            || memories.len() + observations.len() > MAX_AGENTIC_OBSERVATIONS
        {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let _delivery = turn
            .delivery
            .try_lock()
            .map_err(|_| GatewayError::HostStatusUnknown)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let ids = self.validate_inspected_scoped_selection(turn, memories, observations)?;
        state
            .turn
            .confirm_selected(
                ids.into_iter()
                    .chain(observations.iter().map(observation_retrieval_id)),
            )
            .map_err(|reason| GatewayError::Retrieval(reason.into()))?;
        state
            .provided_observations
            .extend(observations.iter().copied());
        state.generation = state.generation.saturating_add(1);
        Ok(())
    }
}

/// Remap only internal turn keys. Model requests, result references and host
/// manifests retain the original Memory ID and its native owner.
pub(super) fn phy_memory_key(owner: [u8; 16], id: MemoryId) -> MemoryId {
    let mut key = Sha256::new();
    key.update(b"reliquary/phy-recall-memory/v1\0");
    key.update(owner);
    key.update(id.0);
    MemoryId(key.finalize().into())
}

#[derive(PartialEq)]
pub(super) struct ScopedRecall {
    rel: Option<OwnerRecall>,
    phy: OwnerRecall,
    phy_cut: PhyAuditCut,
}

#[derive(PartialEq)]
struct OwnerRecall {
    snapshot: RelRetrievalSnapshot,
    profile: Option<CompatibilityProfileId>,
    publication: u64,
    removal_cut: u64,
    observations: BTreeMap<MemoryId, ObservationRetrievalCandidate>,
}

pub(super) struct ScopedEvaluation {
    pub evaluation: RetrievalEvaluation,
    pub preview: RetrievalPreview,
    pub phy_memories: Vec<RetrievalMemoryReference>,
    pub rel_memory: RelRetrievalSnapshot,
    pub removal_cut: u64,
}

impl ScopedRecall {
    pub(super) fn profile_has_vectors(&self) -> bool {
        self.owners().any(|owner| {
            !owner.snapshot.vectors.is_empty()
                || owner
                    .observations
                    .values()
                    .any(|row| row.input.candidate.vector.is_some())
        })
    }

    fn owners(&self) -> impl Iterator<Item = &OwnerRecall> {
        self.rel.iter().chain(std::iter::once(&self.phy))
    }

    pub(super) fn evaluate(
        &self,
        endpoint: &dyn DecisionEndpoint,
        prior: &RetrievalTurn,
        action: RetrievalAction,
        query_vector: Option<&[f32]>,
        query_filter: Option<TemporalFilter>,
        ranking: RetrievalConfig,
    ) -> Result<ScopedEvaluation, GatewayError> {
        let primary = self.rel.as_ref().unwrap_or(&self.phy);
        let mut combined = primary.snapshot.clone();
        let phy = &self.phy.snapshot;
        let mut identities = BTreeMap::new();
        let mut phy_keys = BTreeMap::new();
        if self.rel.is_none() {
            combined.memories.clear();
            combined.active_relations.clear();
            combined.vectors.clear();
        }
        for row in &phy.memories {
            let mut row = row.clone();
            let id = row.revision.record.id;
            let key = phy_memory_key(phy.owner_id, id);
            identities.insert(key, (phy.owner_id, id));
            phy_keys.insert(key, id);
            row.revision.record.id = key;
            let original_body = row.revision.record.body_id;
            let body_key = MemoryBodyId(phy_memory_key(phy.owner_id, MemoryId(original_body.0)).0);
            if let Some(vector) = phy.vectors.get(&original_body) {
                combined.vectors.insert(body_key, vector.clone());
            }
            row.revision.record.body_id = body_key;
            combined.memories.push(row);
        }
        combined
            .active_relations
            .extend(phy.active_relations.iter().filter_map(|relation| {
                let (NodeRef::Memory(source), NodeRef::Memory(target)) =
                    (relation.source, relation.target)
                else {
                    return None;
                };
                let mut relation = *relation;
                relation.source = NodeRef::Memory(phy_memory_key(phy.owner_id, MemoryId(source)).0);
                relation.target = NodeRef::Memory(phy_memory_key(phy.owner_id, MemoryId(target)).0);
                Some(relation)
            }));
        let observations = self
            .owners()
            .flat_map(|owner| owner.observations.iter())
            .collect::<BTreeMap<_, _>>();
        for (&&key, row) in &observations {
            identities.insert(key, (row.result.claim.citation.owner_id, key));
        }
        let observations_input = observations
            .values()
            .map(|row| row.input.clone())
            .collect::<Vec<_>>();
        let mut evaluation = evaluate_scoped_snapshot_retrieval(
            endpoint,
            prior,
            &combined,
            action,
            query_vector,
            query_filter,
            &observations_input,
            Some(&AgenticScope {
                identities: &identities,
                ranking,
            }),
        )
        .map_err(GatewayError::Retrieval)?;
        let mut ranked = Vec::new();
        let mut memories = Vec::new();
        let mut phy_memories = Vec::new();
        let mut selected_observations = Vec::new();
        let mut temporal_evidence = BTreeMap::new();
        for row in &evaluation.memories {
            let scores = evaluation.scores.get(&row.id);
            if let Some(observation) = observations.get(&row.id) {
                let mut result = observation.result.clone();
                if let Some(scores) = scores {
                    result.lexical_score = scores.lexical;
                    result.semantic_score = scores.semantic;
                    result.combined_score = Some(scores.combined);
                }
                ranked.push(RetrievalRankedHit::Observation {
                    citation: result.claim.citation,
                });
                selected_observations.push(result);
                continue;
            }
            let (owner, id) = if let Some(id) = phy_keys.get(&row.id) {
                (phy, *id)
            } else {
                (&primary.snapshot, row.id)
            };
            let mut result = project_memories(owner, [id])?
                .pop()
                .ok_or(GatewayError::StaleRetrieval)?;
            if let Some(scores) = scores {
                result.lexical_score = (scores.lexical > 0.0).then_some(scores.lexical);
                result.semantic_score = scores.semantic;
                result.combined_score = Some(scores.combined);
            }
            let reference = result.reference();
            if phy_keys.contains_key(&row.id) {
                phy_memories.push(reference.clone());
            }
            ranked.push(RetrievalRankedHit::Memory { reference });
            if let Some(evidence) = evaluation.temporal_evidence.get(&row.id) {
                temporal_evidence.insert(
                    format!("{}:{}", result.owner_id, result.memory_id),
                    evidence.clone(),
                );
            }
            memories.push(result);
        }
        evaluation
            .memories
            .retain(|row| !observations.contains_key(&row.id));
        evaluation
            .already_provided
            .retain(|id| !observations.contains_key(id));
        let mut phy_cut = RetrievalCut::capture(phy, self.phy.profile, self.phy.publication);
        phy_cut.owner_kind = "phylactery";
        let preview = RetrievalPreview {
            cut: self.rel.as_ref().map_or_else(
                || phy_cut.clone(),
                |rel| RetrievalCut::capture(&rel.snapshot, rel.profile, rel.publication),
            ),
            phy_cut: Some(phy_cut),
            ranked,
            query: evaluation.next.query().unwrap_or_default().to_owned(),
            temporal_filter: evaluation.next.temporal_filter(),
            temporal_evidence: evaluation.next.temporal_filter().map(|_| temporal_evidence),
            truncated: memories.iter().any(|memory| memory.truncated)
                || !evaluation.temporal_evidence_truncated.is_empty(),
            memories,
            observations: selected_observations,
        };
        Ok(ScopedEvaluation {
            evaluation,
            preview,
            phy_memories,
            rel_memory: primary.snapshot.clone(),
            removal_cut: primary.removal_cut,
        })
    }
}
