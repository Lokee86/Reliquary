//! Frozen semantic-only acceptance cases. No store or owner write handles.
use super::*;
struct FrozenEndpoint {
    response: Value,
}
impl FrozenEndpoint {
    fn new(response: Value) -> Self {
        Self { response }
    }
}
impl GeneralEndpoint for FrozenEndpoint {
    fn model(&self) -> &str {
        "frozen-dream-evidence-v1"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(self.response.clone())
    }
}

fn memory(id: u8, at: i64) -> DreamMemory {
    DreamMemory {
        id: MemoryId([id; 32]),
        body_id: MemoryBodyId([id; 32]),
        category: "fact".into(),
        memory_type: "project".into(),
        lifecycle_state: "knowledge".into(),
        title: "Retention policy".into(),
        content: "Retention policy is defined by this document.".into(),
        source_timestamp_ns: Some(at),
        temporal: json!({"anchors":[],"patterns":[]}),
        graph_relations: Vec::new(),
    }
}
fn fact(memory: &DreamMemory) -> MemoryFact {
    MemoryFact {
        id: memory.id,
        lifecycle_state: memory.lifecycle_state.clone(),
        archived: false,
        superseded_by: None,
        source_time_ns: memory.source_timestamp_ns,
        authority_kind: "inferred".into(),
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
    }
}
fn duplicate_classification() -> Value {
    json!({
        "relation":"duplicate_of",
        "direction":"undirected",
        "evidence":[
            {"side":"a","quote":"Retention policy"},
            {"side":"b","quote":"Retention policy"}
        ]
    })
}

#[test]
fn frozen_duplicate_classification_requires_acceptance_and_plans_no_writes_directly() {
    let a = memory(1, 10);
    let b = memory(2, 20);
    let evaluator = DreamPairEvaluator::new(
        FrozenEndpoint::new(duplicate_classification()),
        FrozenEndpoint::new(json!({
            "relation_supported":"yes",
            "direction_supported":"yes",
            "evidence_supported":"yes"
        })),
    );
    let (classification, verification) = evaluator
        .evaluate_pair_with_retries(DreamVerificationPolicy::default(), &b, &a)
        .unwrap();
    assert_eq!((classification.a, classification.b), (a.id, b.id));
    assert_eq!(classification.relation, DreamRelationKind::DuplicateOf);
    assert_eq!(classification.direction, DreamRelationDirection::Undirected);
    assert_eq!(classification.evidence.len(), 2);
    assert_eq!(
        verification.unwrap().verdict,
        DreamVerificationVerdict::Accept
    );
    let plan = evaluate_snapshot(
        &[PairDecision {
            a: classification.a,
            b: classification.b,
            relation: PairRelation::DuplicateOf,
            direction: PairDirection::Undirected,
        }],
        &[fact(&a), fact(&b)],
        &[],
    );
    assert_eq!(plan.lifecycle_intents.len(), 0);
    assert_eq!(
        plan.relation_changes,
        vec![RelationChange {
            source: NodeRef::Memory(b.id.0),
            target: NodeRef::Memory(a.id.0),
            kind: RelationKind::DuplicateOf,
            active: true,
            origin: RelationOrigin::Dream,
        }]
    );
}

#[test]
fn frozen_rejected_duplicate_is_withheld_from_all_semantic_effects() {
    let a = memory(1, 10);
    let b = memory(2, 20);
    let evaluator = DreamPairEvaluator::new(
        FrozenEndpoint::new(duplicate_classification()),
        FrozenEndpoint::new(json!({
            "relation_supported":"no",
            "direction_supported":"yes",
            "evidence_supported":"yes"
        })),
    );
    let (classification, verification) = evaluator
        .evaluate_pair_with_retries(DreamVerificationPolicy::default(), &a, &b)
        .unwrap();
    assert_eq!(classification.relation, DreamRelationKind::DuplicateOf);
    assert_eq!(
        verification.unwrap().verdict,
        DreamVerificationVerdict::Reject
    );
    let plan = evaluate_snapshot(&[], &[fact(&a), fact(&b)], &[]);
    assert!(plan.relation_changes.is_empty());
    assert!(plan.lifecycle_intents.is_empty());
}

#[test]
fn frozen_zero_link_classification_returns_no_graph_or_lifecycle_effect() {
    let a = memory(1, 10);
    let b = memory(2, 20);
    let classifier = DreamClassifier::new(FrozenEndpoint::new(json!({
        "relation":"none",
        "direction":"none",
        "evidence":[]
    })));
    let classification = classifier.classify_pair(&a, &b).unwrap();
    assert_eq!(classification.relation, DreamRelationKind::None);
    let verifier = DreamVerifier::new(FrozenEndpoint::new(json!({
        "relation_supported":"yes",
        "direction_supported":"yes",
        "evidence_supported":"yes"
    })));
    assert!(
        verifier
            .verify_if_required(DreamVerificationPolicy::default(), &classification, &a, &b)
            .unwrap()
            .is_none()
    );
    let plan = evaluate_snapshot(
        &[PairDecision {
            a: a.id,
            b: b.id,
            relation: PairRelation::None,
            direction: PairDirection::None,
        }],
        &[fact(&a), fact(&b)],
        &[],
    );
    assert!(plan.relation_changes.is_empty());
    assert!(plan.lifecycle_intents.is_empty());

    // Zero-link is still a valid first-cycle result. Dream's pure lifecycle
    // policy can advance extracted knowledge without fabricating a Graph link.
    let mut extracted = fact(&a);
    extracted.lifecycle_state = "extracted".into();
    assert_eq!(
        plan_lifecycle(&extracted, &[], false, &[], false),
        vec![LifecycleIntent {
            memory_id: a.id,
            lifecycle_state: "knowledge".into(),
            archived: false,
            superseded_by: None,
        }]
    );
}
