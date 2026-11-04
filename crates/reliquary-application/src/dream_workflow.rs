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
    pub owner_id: [u8; 16],
    pub graph_version: u64,
    pub observed_memories: Vec<DreamMemoryObservation>,
    pub classifications: Vec<DreamPairClassification>,
    pub verifications: Vec<DreamPairVerification>,
    /// Pure proposal from Dream's snapshot evaluator; it is not persisted.
    pub plan: EvaluationPlan,
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
        let history = owner
            .memory_history(id)
            .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?
            .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
        if history.tombstone.is_some() {
            return Err(DreamWorkflowError::MemoryTombstoned(id));
        }
        let revision = history
            .revisions
            .last()
            .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
        let body = match &revision.payload {
            PayloadState::Available(body) => body,
            PayloadState::Absent => return Err(DreamWorkflowError::MemoryBodyUnavailable(id)),
        };
        let record = &revision.record;
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
    let set = CandidateSet {
        source,
        candidates: memories.iter().skip(1).cloned().collect(),
    };
    let classifications = classifier
        .classify_candidates(&set)
        .map_err(|error| DreamWorkflowError::Classifier(error.to_string()))?;
    let by_id: HashMap<_, _> = memories.iter().map(|memory| (memory.id, memory)).collect();
    let mut verifications = Vec::new();
    let mut accepted_decisions = Vec::new();
    for classification in &classifications {
        if verification_policy.should_verify(classification.relation) {
            let Some(verifier) = verifier else {
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
                continue;
            };
            let accepted = verification.verdict == DreamVerificationVerdict::Accept;
            verifications.push(verification);
            if !accepted {
                continue;
            }
        }
        accepted_decisions.push(pair_decision(classification));
    }
    let plan = reliquary_dream::evaluate_snapshot(
        &accepted_decisions,
        &facts,
        &existing_relations.into_values().collect::<Vec<_>>(),
    );
    Ok(DreamOrganizationDraft {
        owner_id,
        graph_version: owner.graph_version(),
        observed_memories: observations,
        classifications,
        verifications,
        plan,
    })
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
