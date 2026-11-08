//! Owner-local, read-only Dream organization proposal preparation.

use crate::EntityOwnerSession;
use reliquary_dream::{
    DreamCandidateContexts, DreamClassifier, DreamMemory, DreamPairClassification,
    DreamPairVerification, DreamRelationDirection, DreamRelationKind, DreamVerificationPolicy,
    DreamVerificationVerdict, DreamVerifier, EvaluationPlan, MemoryFact, PairDecision,
    PairDirection, PairRelation,
};
use reliquary_owner_session::model::{
    MemoryBodyId, MemoryId, MemoryTemporalInference, NodeRef, PayloadState, Relation, RelationKind,
    RelationOrigin,
};
use reliquary_switchboard::GeneralEndpoint;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

pub const MAX_DREAM_CANDIDATES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamMemoryObservation {
    pub memory_id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DreamWorkflowError {
    OwnerMismatch,
    CandidateLimit,
    DuplicateMemory,
    MemoryUnavailable(MemoryId),
    MemoryTombstoned(MemoryId),
    MemoryBodyUnavailable(MemoryId),
    MissingSourceVector,
    InvalidVector(String),
    TemporalInferenceUnavailable,
    UnsupportedPublication,
    IncompleteFrontier,
    OwnerWrite(String),
    Classifier(String),
    Verifier(String),
    StaleSnapshot,
    OwnerRead(String),
}

impl fmt::Display for DreamWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("Dream owner does not match caller authorization"),
            Self::CandidateLimit => f.write_str("Dream candidate limit exceeded"),
            Self::DuplicateMemory => f.write_str("Dream input repeats a Memory identity"),
            Self::MemoryUnavailable(id) => write!(f, "Memory {id:?} is unavailable in owner"),
            Self::MemoryTombstoned(id) => write!(f, "Memory {id:?} is tombstoned"),
            Self::MemoryBodyUnavailable(id) => write!(f, "Memory {id:?} body is unavailable"),
            Self::MissingSourceVector => f.write_str("Dream source embedding is unavailable"),
            Self::InvalidVector(error) => write!(f, "Dream vector is invalid: {error}"),
            Self::TemporalInferenceUnavailable => {
                f.write_str("Dream temporal inference failed source/contract validation")
            }
            Self::IncompleteFrontier => f.write_str(
                "Dream frontier has pending or uncertain decisions; retry before settlement",
            ),
            Self::UnsupportedPublication => {
                f.write_str("Dream publication requires a single evaluated zero-link decision")
            }
            Self::OwnerWrite(error) => write!(f, "Dream owner publication failed: {error}"),
            Self::Classifier(error) => write!(f, "Dream classification failed: {error}"),
            Self::Verifier(error) => write!(f, "Dream verification failed: {error}"),
            Self::StaleSnapshot => f.write_str("Dream proposal no longer matches owner state"),
            Self::OwnerRead(error) => write!(f, "Dream owner read failed: {error}"),
        }
    }
}

impl std::error::Error for DreamWorkflowError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamOrganizationDraft {
    // Only the checked Application workflow may create or change this draft.
    // External callers can inspect but cannot forge or replace its model data.
    pub(crate) owner_id: [u8; 16],
    pub(crate) graph_version: u64,
    pub(crate) observed_memories: Vec<DreamMemoryObservation>,
    pub(crate) classifications: Vec<DreamPairClassification>,
    pub(crate) verifications: Vec<DreamPairVerification>,
    /// Pure proposal from Dream's snapshot evaluator; it is not persisted.
    pub(crate) plan: EvaluationPlan,
    pub(crate) accepted_decisions: Vec<PairDecision>,
    pub(crate) pending: Vec<(MemoryId, MemoryId)>,
    pub(crate) skipped: Vec<MemoryId>,
    pub(crate) batch_ready: bool,
    pub(crate) facts: Vec<MemoryFact>,
    pub(crate) relations: Vec<Relation>,
}

impl DreamOrganizationDraft {
    pub fn pending_pairs(&self) -> &[(MemoryId, MemoryId)] {
        &self.pending
    }
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
    pub fn graph_version(&self) -> u64 {
        self.graph_version
    }
    pub fn observed_memories(&self) -> &[DreamMemoryObservation] {
        &self.observed_memories
    }
    pub fn classifications(&self) -> &[DreamPairClassification] {
        &self.classifications
    }
    pub fn verifications(&self) -> &[DreamPairVerification] {
        &self.verifications
    }
    pub fn plan(&self) -> &EvaluationPlan {
        &self.plan
    }
}

struct CandidateSet {
    source: DreamMemory,
    candidates: Vec<DreamMemory>,
}

impl DreamCandidateContexts for CandidateSet {
    type Context = DreamMemory;

    fn source(&self) -> &Self::Context {
        &self.source
    }

    fn candidates(&self) -> Vec<&Self::Context> {
        self.candidates.iter().collect()
    }
}

/// Prepare an owner-local Dream graph/lifecycle proposal from current stored
/// Memory facts. The owner session must have been obtained after caller-side
/// authorization. Classifier evidence is checked by Dream against exact body
/// text. Verification failures and missing required verifiers are omitted from
/// the plan, so they cannot create or remove relations.
pub fn draft_dream_organization<C: GeneralEndpoint, V: GeneralEndpoint>(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    candidate_ids: &[MemoryId],
    classifier: &DreamClassifier<C>,
    verifier: Option<&DreamVerifier<V>>,
    verification_policy: DreamVerificationPolicy,
) -> Result<DreamOrganizationDraft, DreamWorkflowError> {
    prepare_dream_organization(owner, authorized_owner_id, source_id, candidate_ids)?.evaluate(
        classifier,
        verifier,
        verification_policy,
    )
}

pub(crate) struct PreparedDreamOrganization {
    owner_id: [u8; 16],
    graph_version: u64,
    facts: Vec<MemoryFact>,
    observations: Vec<DreamMemoryObservation>,
    memories: Vec<DreamMemory>,
    set: CandidateSet,
    existing_relations: BTreeMap<(MemoryId, MemoryId, RelationKind), Relation>,
    skipped: Vec<MemoryId>,
}

pub(crate) fn prepare_dream_organization(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    candidate_ids: &[MemoryId],
) -> Result<PreparedDreamOrganization, DreamWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != authorized_owner_id {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    if candidate_ids.len() > MAX_DREAM_CANDIDATES {
        return Err(DreamWorkflowError::CandidateLimit);
    }
    let mut ids = Vec::with_capacity(candidate_ids.len() + 1);
    ids.push(source_id);
    ids.extend_from_slice(candidate_ids);
    let mut unique = HashSet::with_capacity(ids.len());
    if ids.iter().any(|id| !unique.insert(*id)) {
        return Err(DreamWorkflowError::DuplicateMemory);
    }

    let mut facts = Vec::with_capacity(ids.len());
    let mut observations = Vec::with_capacity(ids.len());
    let mut memories = Vec::with_capacity(ids.len());
    for id in ids {
        let revision = owner.current_memory(id).map_err(|error| match error {
            reliquary_owner_session::OwnerSessionError::Memory(
                reliquary_owner_session::model::MemoryError::TerminalTombstone,
            ) => DreamWorkflowError::MemoryTombstoned(id),
            _ => DreamWorkflowError::MemoryUnavailable(id),
        })?;
        let body = match &revision.payload {
            PayloadState::Available(body) => body,
            PayloadState::Absent => return Err(DreamWorkflowError::MemoryBodyUnavailable(id)),
        };
        let record = &revision.record;
        crate::dream_temporal::analyze(record, body)?;
        let fact = MemoryFact {
            id,
            lifecycle_state: record.lifecycle_state.clone(),
            archived: record.archived,
            superseded_by: record.superseded_by,
            source_time_ns: record.source_time_ns,
            authority_kind: record.authority_kind.clone(),
            category: record.category.clone(),
            memory_type: record.memory_type.clone(),
        };
        observations.push(DreamMemoryObservation {
            memory_id: id,
            revision: record.revision,
            body_id: record.body_id,
        });
        facts.push(fact);
        memories.push(DreamMemory {
            id,
            body_id: record.body_id,
            category: record.category.clone(),
            memory_type: record.memory_type.clone(),
            lifecycle_state: record.lifecycle_state.clone(),
            title: body.title.clone(),
            content: body.content.clone(),
            source_timestamp_ns: record.source_time_ns,
            temporal: temporal_json(record.temporal_inference.as_ref()),
            graph_relations: Vec::new(),
        });
    }

    let mut existing_relations = BTreeMap::new();
    let graph_kinds = [
        RelationKind::Topical,
        RelationKind::Factual,
        RelationKind::Causal,
        RelationKind::Recurrent,
        RelationKind::DuplicateOf,
        RelationKind::Supersedes,
    ];
    let mut graph_context: HashMap<MemoryId, Vec<Value>> = HashMap::new();
    for candidate in candidate_ids {
        for kind in graph_kinds {
            for (source, target) in [(source_id, *candidate), (*candidate, source_id)] {
                if let Some(relation) =
                    owner.graph_relation(NodeRef::Memory(source.0), NodeRef::Memory(target.0), kind)
                {
                    let key = (source, target, kind);
                    existing_relations.insert(key, relation.clone());
                    let value = relation_json(&relation);
                    graph_context.entry(source).or_default().push(value.clone());
                    graph_context.entry(target).or_default().push(value);
                }
            }
        }
    }
    for memory in &mut memories {
        memory.graph_relations = graph_context.remove(&memory.id).unwrap_or_default();
    }

    let source = memories
        .first()
        .cloned()
        .ok_or(DreamWorkflowError::MemoryUnavailable(source_id))?;
    let excluded: HashSet<_> = existing_relations
        .values()
        .filter(|relation| relation.active)
        .flat_map(|relation| [relation.source, relation.target])
        .filter_map(|node| match node {
            NodeRef::Memory(id) if id != source_id.0 => Some(MemoryId(id)),
            _ => None,
        })
        .chain(
            facts
                .iter()
                .filter(|fact| fact.archived)
                .map(|fact| fact.id),
        )
        .collect();
    let skipped: Vec<_> = memories
        .iter()
        .skip(1)
        .filter(|m| excluded.contains(&m.id) && !owner.dream_pair_evaluated(source_id, m.id))
        .map(|m| m.id)
        .collect();
    let set = CandidateSet {
        source,
        candidates: memories
            .iter()
            .skip(1)
            .filter(|memory| {
                !excluded.contains(&memory.id) && !owner.dream_pair_evaluated(source_id, memory.id)
            })
            .cloned()
            .collect(),
    };
    Ok(PreparedDreamOrganization {
        owner_id,
        graph_version: owner.graph_version(),
        facts,
        observations,
        memories,
        set,
        existing_relations,
        skipped,
    })
}

impl PreparedDreamOrganization {
    pub(crate) fn evaluate<C: GeneralEndpoint, V: GeneralEndpoint>(
        self,
        classifier: &DreamClassifier<C>,
        verifier: Option<&DreamVerifier<V>>,
        verification_policy: DreamVerificationPolicy,
    ) -> Result<DreamOrganizationDraft, DreamWorkflowError> {
        let Self {
            owner_id,
            graph_version,
            facts,
            observations,
            memories,
            set,
            existing_relations,
            skipped,
        } = self;
        let classifications = classifier
            .classify_candidates(&set)
            .map_err(|error| DreamWorkflowError::Classifier(error.to_string()))?;
        let by_id: HashMap<_, _> = memories.iter().map(|memory| (memory.id, memory)).collect();
        let mut verifications = Vec::new();
        let mut accepted_decisions = Vec::new();
        let mut pending = Vec::new();
        for classification in &classifications {
            if verification_policy.should_verify(classification.relation) {
                let Some(verifier) = verifier else {
                    pending.push((classification.a, classification.b));
                    continue;
                };
                let left = by_id
                    .get(&classification.a)
                    .ok_or(DreamWorkflowError::MemoryUnavailable(classification.a))?;
                let right = by_id
                    .get(&classification.b)
                    .ok_or(DreamWorkflowError::MemoryUnavailable(classification.b))?;
                let Some(verification) = verifier
                    .verify_if_required(verification_policy, classification, *left, *right)
                    .map_err(|error| DreamWorkflowError::Verifier(error.to_string()))?
                else {
                    pending.push((classification.a, classification.b));
                    continue;
                };
                if verification.verdict == DreamVerificationVerdict::Uncertain {
                    pending.push((classification.a, classification.b));
                }
                let accepted = verification.verdict == DreamVerificationVerdict::Accept;
                verifications.push(verification);
                if !accepted {
                    continue;
                }
            }
            accepted_decisions.push(pair_decision(classification));
        }
        let relations = existing_relations.into_values().collect::<Vec<_>>();
        let plan = reliquary_dream::evaluate_snapshot(&accepted_decisions, &facts, &relations);
        Ok(DreamOrganizationDraft {
            owner_id,
            graph_version,
            observed_memories: observations,
            classifications,
            verifications,
            plan,
            accepted_decisions,
            pending,
            skipped,
            batch_ready: false,
            facts,
            relations,
        })
    }
}

/// Recheck the owner and source revisions immediately before an eventual
/// commit. This does not commit the plan or authorize host access.
pub fn recheck_dream_draft(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    draft: &DreamOrganizationDraft,
) -> Result<(), DreamWorkflowError> {
    if owner.owner_id() != authorized_owner_id || draft.owner_id != authorized_owner_id {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    if owner.graph_version() != draft.graph_version {
        return Err(DreamWorkflowError::StaleSnapshot);
    }
    if draft
        .classifications
        .iter()
        .any(|pair| owner.dream_pair_evaluated(pair.a, pair.b))
    {
        return Err(DreamWorkflowError::StaleSnapshot);
    }
    for observed in &draft.observed_memories {
        let history = owner
            .memory_history(observed.memory_id)
            .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?
            .ok_or(DreamWorkflowError::StaleSnapshot)?;
        let Some(current) = history.revisions.last() else {
            return Err(DreamWorkflowError::StaleSnapshot);
        };
        if history.tombstone.is_some()
            || current.record.revision != observed.revision
            || current.record.body_id != observed.body_id
        {
            return Err(DreamWorkflowError::StaleSnapshot);
        }
    }
    Ok(())
}

fn pair_decision(classification: &DreamPairClassification) -> PairDecision {
    PairDecision {
        a: classification.a,
        b: classification.b,
        relation: match classification.relation {
            DreamRelationKind::None => PairRelation::None,
            DreamRelationKind::Topical => PairRelation::Topical,
            DreamRelationKind::Factual => PairRelation::Factual,
            DreamRelationKind::Causal => PairRelation::Causal,
            DreamRelationKind::Recurrent => PairRelation::Recurrent,
            DreamRelationKind::DuplicateOf => PairRelation::DuplicateOf,
            DreamRelationKind::Supersedes => PairRelation::Supersedes,
        },
        direction: match classification.direction {
            DreamRelationDirection::None => PairDirection::None,
            DreamRelationDirection::Undirected => PairDirection::Undirected,
            DreamRelationDirection::AToB => PairDirection::AToB,
            DreamRelationDirection::BToA => PairDirection::BToA,
        },
    }
}

fn temporal_json(value: Option<&MemoryTemporalInference>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    json!({
        "body_id": value.body_id.0,
        "source_time_ns": value.source_time_ns,
        "model": value.inference.model,
        "contract_version": value.inference.contract_version,
        "resolutions": value.inference.resolutions.iter().map(|item| json!({
            "start_byte": item.start_byte,
            "end_byte": item.end_byte,
            "kind": format!("{:?}", item.kind),
            "evidence": item.evidence,
            "canonical_expression": item.canonical_expression,
        })).collect::<Vec<_>>(),
    })
}

fn relation_json(relation: &Relation) -> Value {
    json!({
        "source": format!("{:?}", relation.source),
        "target": format!("{:?}", relation.target),
        "kind": format!("{:?}", relation.kind),
        "active": relation.active,
        "origin": match relation.origin {
            RelationOrigin::User => "user",
            RelationOrigin::Dream => "dream",
            RelationOrigin::Perception => "perception",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityOwnerSession;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_dream::DreamVerificationPolicy;
    use reliquary_memory_core::{MemoryDraft, MemoryStore};
    use reliquary_owner_session::{GraphAdmission, PhySession};
    use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
    use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        sync::{Arc, Mutex},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "dream-workflow-{}-{}.phy",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[derive(Clone, Default)]
    struct RecordingEndpoint(Arc<Mutex<Vec<String>>>);
    impl GeneralEndpoint for RecordingEndpoint {
        fn model(&self) -> &str {
            "dream-test"
        }
        fn complete_json(
            &self,
            _prompt: &str,
            payload: &str,
            _schema_name: &str,
            _schema: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            self.0.lock().unwrap().push(payload.to_owned());
            Ok(json!({
                "relation":"topical",
                "direction":"undirected",
                "evidence":[
                    {"side":"a","quote":"Rollout"},
                    {"side":"b","quote":"Rollout"}
                ]
            }))
        }
    }

    fn memory_draft(mutation: &str, title: &str, content: &str) -> MemoryDraft {
        MemoryDraft {
            title: title.into(),
            content: content.into(),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(1),
            temporal_inference: None,
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        }
    }

    fn owner() -> (Temp, PhySession, [u8; 16], MemoryId, MemoryId) {
        let path = Temp::new();
        let owner_id = [103; 16];
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut store = MemoryStore::create(&mut container).unwrap();
        let a = store
            .publish(
                &mut container,
                None,
                0,
                memory_draft(
                    "dream-a",
                    "Rollout notes",
                    "The rollout uses a guarded migration path.",
                ),
            )
            .unwrap()
            .revision
            .record
            .id;
        let b = store
            .publish(
                &mut container,
                None,
                0,
                memory_draft(
                    "dream-b",
                    "Rollout plan",
                    "The rollout has a staged deployment schedule.",
                ),
            )
            .unwrap()
            .revision
            .record
            .id;
        let unrelated = store
            .publish(
                &mut container,
                None,
                0,
                memory_draft(
                    "dream-private",
                    "Unselected private note",
                    "UNSELECTED-PRIVATE-OWNER-FACT",
                ),
            )
            .unwrap()
            .revision
            .record
            .id;
        container.sync().unwrap();
        drop(store);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        assert_ne!(unrelated, a);
        (path, session, owner_id, a, b)
    }

    fn dream_test_profile() -> reliquary_vector_store::CompatibilityProfile {
        use reliquary_vector_store::{
            CompatibilityProbeReference, EmbeddingMode, VectorNormalization,
        };
        let mut profile = reliquary_vector_store::CompatibilityProfile {
            id: reliquary_vector_store::CompatibilityProfileId([0; 32]),
            dimensions: 2,
            normalization: VectorNormalization::L2,
            probe_suite_version: 1,
            compatibility_policy_version: 2,
            references: [
                EmbeddingMode::Query,
                EmbeddingMode::Query,
                EmbeddingMode::Document,
                EmbeddingMode::Document,
            ]
            .into_iter()
            .map(|mode| CompatibilityProbeReference {
                mode,
                vector: vec![1.0, 0.0],
            })
            .collect(),
        };
        profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
        profile
    }

    #[test]
    fn owner_candidate_ranking_uses_profile_vectors_and_pair_history() {
        use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
        use reliquary_vector_store::AcceptedMemoryMapping;
        let (path, session, owner_id, source, candidate) = owner();
        let (_, ids) = session.memory_ids_page(None, 128).unwrap();
        let unrelated = *ids
            .iter()
            .find(|id| **id != source && **id != candidate)
            .unwrap();
        drop(session);
        let mut container = Container::open_writer(&path.0).unwrap();
        let memories = MemoryStore::open(&container).unwrap();
        let bodies = [source, candidate, unrelated]
            .map(|id| memories.history(id).unwrap().revisions[0].record.body_id);
        let mut packed = PackedVectorStore::open(&container).unwrap();
        let values = [1.0f32, 0.0, 0.8, 0.6, 0.0, 1.0];
        let vector = PackedVectors::from_bytes(
            VectorSchema::new(2, ScalarType::F32).unwrap(),
            values.into_iter().flat_map(f32::to_le_bytes).collect(),
        )
        .unwrap();
        let packed_id = packed.put(&mut container, vector).unwrap();
        let mut profiles = CompatibilityProfileStore::open(&container).unwrap();
        let profile = dream_test_profile();
        profiles.put(&mut container, profile.clone()).unwrap();
        let mut vectors = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
        vectors
            .put(
                &mut container,
                &packed,
                &profiles,
                AcceptedMemoryMapping {
                    profile: profile.id,
                    packed: packed_id,
                    packed_rows: 3,
                    body_ids: bodies
                        .into_iter()
                        .map(|body| reliquary_vector_store::MemoryBodyId(body.0))
                        .collect(),
                },
            )
            .unwrap();
        container.sync().unwrap();
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let before = fs::read(&path.0).unwrap();
        let selection = crate::ranked_owner_dream_candidates(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            source,
            profile.id,
            reliquary_dream::DreamCandidateConfig::default(),
        )
        .unwrap();
        assert_eq!(selection.first(), Some(&candidate));
        assert!(selection.contains(&unrelated));
        let endpoint = RecordingEndpoint::default();
        let config = reliquary_dream::DreamCandidateConfig {
            limit: 1,
            prior_semantic_quota: 0,
            ..Default::default()
        };
        let proposal = crate::draft_ranked_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            source,
            profile.id,
            config,
            &DreamClassifier::new(endpoint.clone()),
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(proposal.classifications.len(), 1);
        assert!(
            proposal.classifications[0].a == candidate
                || proposal.classifications[0].b == candidate
        );
        assert_eq!(endpoint.0.lock().unwrap().len(), 1);
        assert_eq!(fs::read(&path.0).unwrap(), before);
        drop(session);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        writer.record_dream_pair(source, candidate).unwrap();
        drop(writer);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let selection = crate::ranked_owner_dream_candidates(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            source,
            profile.id,
            reliquary_dream::DreamCandidateConfig::default(),
        )
        .unwrap();
        assert!(!selection.contains(&candidate));
    }

    #[test]
    fn owner_ranked_candidates_reject_mismatched_authority_without_access() {
        let (path, session, owner_id, source, _) = owner();
        let bytes = fs::read(&path.0).unwrap();
        assert_eq!(
            crate::ranked_owner_dream_candidates(
                &EntityOwnerSession::Phy(&session),
                [owner_id[0].wrapping_add(1); 16],
                source,
                reliquary_owner_session::model::CompatibilityProfileId([0; 32]),
                reliquary_dream::DreamCandidateConfig::default(),
            ),
            Err(DreamWorkflowError::OwnerMismatch)
        );
        assert_eq!(fs::read(&path.0).unwrap(), bytes);
    }

    #[test]
    fn owner_ranked_candidates_require_a_retained_source_embedding() {
        let (path, session, owner_id, source, _) = owner();
        drop(session);
        let mut container = Container::open_writer(&path.0).unwrap();
        let mut profiles = CompatibilityProfileStore::open(&container).unwrap();
        let profile = dream_test_profile();
        profiles.put(&mut container, profile.clone()).unwrap();
        container.sync().unwrap();
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let bytes = fs::read(&path.0).unwrap();
        assert_eq!(
            crate::ranked_owner_dream_candidates(
                &EntityOwnerSession::Phy(&session),
                owner_id,
                source,
                profile.id,
                reliquary_dream::DreamCandidateConfig::default(),
            ),
            Err(DreamWorkflowError::MissingSourceVector)
        );
        assert_eq!(fs::read(&path.0).unwrap(), bytes);
    }

    #[test]
    fn owner_memory_pages_are_stable_bounded_and_clocked() {
        let (_path, session, _owner_id, a, b) = owner();
        let (clock, first) = session.memory_ids_page(None, 2).unwrap();
        let (next_clock, second) = session.memory_ids_page(first.last().copied(), 2).unwrap();
        assert_eq!(clock, 3);
        assert_eq!(clock, next_clock);
        let all: Vec<_> = first.into_iter().chain(second).collect();
        assert_eq!(all.len(), 3);
        assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(all.contains(&a) && all.contains(&b));
        assert!(session.memory_ids_page(None, 0).is_err());
        assert!(session.memory_ids_page(None, 129).is_err());
    }

    #[test]
    fn excludes_previously_evaluated_pair_after_owner_reopen() {
        let (path, session, owner_id, a, b) = owner();
        drop(session);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        assert!(writer.record_dream_pair(a, b).unwrap());
        drop(writer);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let endpoint = RecordingEndpoint::default();
        let draft = draft_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            a,
            &[b],
            &DreamClassifier::new(endpoint.clone()),
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        assert!(draft.classifications.is_empty());
        assert!(endpoint.0.lock().unwrap().is_empty());
    }

    #[test]
    fn draft_recheck_rejects_pair_evaluated_after_preparation() {
        let (path, session, owner_id, a, b) = owner();
        let draft = draft_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            a,
            &[b],
            &DreamClassifier::new(RecordingEndpoint::default()),
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        drop(session);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        writer.record_dream_pair(a, b).unwrap();
        drop(writer);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        assert_eq!(
            recheck_dream_draft(&EntityOwnerSession::Phy(&session), owner_id, &draft),
            Err(DreamWorkflowError::StaleSnapshot)
        );
    }

    #[test]
    fn excludes_archived_and_already_related_memories_before_model_call() {
        let (path, session, owner_id, a, b) = owner();
        drop(session);
        let mut container = Container::open_writer(&path.0).unwrap();
        let admission = GraphAdmission::for_changes(owner_id, [a.0, b.0], []);
        let mut graph = reliquary_graph::GraphStore::open(&container, &admission).unwrap();
        graph
            .put(
                &mut container,
                &admission,
                &[reliquary_graph::RelationChange {
                    source: NodeRef::Memory(a.0),
                    target: NodeRef::Memory(b.0),
                    kind: RelationKind::Topical,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        drop(container);
        let replay = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &replay, []).unwrap();
        let endpoint = RecordingEndpoint::default();
        let draft = draft_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            a,
            &[b],
            &DreamClassifier::new(endpoint.clone()),
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        assert!(draft.classifications.is_empty());
        assert!(endpoint.0.lock().unwrap().is_empty());
        drop(session);

        let mut container = Container::open_writer(&path.0).unwrap();
        let mut graph = reliquary_graph::GraphStore::open(&container, &admission).unwrap();
        graph
            .put(
                &mut container,
                &admission,
                &[reliquary_graph::RelationChange {
                    source: NodeRef::Memory(a.0),
                    target: NodeRef::Memory(b.0),
                    kind: RelationKind::Topical,
                    active: false,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        let mut memories = MemoryStore::open(&container).unwrap();
        let mut archived = memory_draft(
            "archive-b",
            "Rollout plan",
            "The rollout has a staged deployment schedule.",
        );
        archived.archived = true;
        memories
            .publish(&mut container, Some(b), 1, archived)
            .unwrap();
        drop(memories);
        drop(container);
        let session = PhySession::open_path(&path.0, &replay, []).unwrap();
        let endpoint = RecordingEndpoint::default();
        let draft = draft_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            a,
            &[b],
            &DreamClassifier::new(endpoint.clone()),
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        assert!(draft.classifications.is_empty());
        assert!(endpoint.0.lock().unwrap().is_empty());
    }

    #[test]
    fn drafts_graph_changes_from_hydrated_owner_facts_and_rechecks_without_writes() {
        let (_path, session, owner_id, a, b) = owner();
        let endpoint = RecordingEndpoint::default();
        let classifier = DreamClassifier::new(endpoint.clone());
        let before_graph_version = session.graph_version();
        let draft = draft_dream_organization(
            &EntityOwnerSession::Phy(&session),
            owner_id,
            a,
            &[b],
            &classifier,
            None::<&DreamVerifier<RecordingEndpoint>>,
            DreamVerificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(draft.owner_id, owner_id);
        assert_eq!(draft.observed_memories.len(), 2);
        assert_eq!(draft.classifications.len(), 1);
        assert_eq!(draft.plan.relation_changes.len(), 1);
        assert!(draft.plan.lifecycle_intents.is_empty());
        assert_eq!(draft.graph_version, before_graph_version);
        assert!(recheck_dream_draft(&EntityOwnerSession::Phy(&session), owner_id, &draft).is_ok());
        assert_eq!(session.graph_version(), before_graph_version);
        assert!(
            session
                .graph_relation(
                    NodeRef::Memory(a.0),
                    NodeRef::Memory(b.0),
                    RelationKind::Topical
                )
                .is_none()
        );

        let payload = endpoint.0.lock().unwrap().join("\n");
        assert!(payload.contains("The rollout uses a guarded migration path."));
        assert!(payload.contains("The rollout has a staged deployment schedule."));
        assert!(!payload.contains("UNSELECTED-PRIVATE-OWNER-FACT"));

        assert_eq!(
            draft_dream_organization(
                &EntityOwnerSession::Phy(&session),
                [104; 16],
                a,
                &[b],
                &classifier,
                None::<&DreamVerifier<RecordingEndpoint>>,
                DreamVerificationPolicy::default(),
            ),
            Err(DreamWorkflowError::OwnerMismatch)
        );
    }
}
