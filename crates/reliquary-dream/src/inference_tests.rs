use super::*;
use serde_json::{Value, json};
use std::sync::Mutex;

struct Stub(Mutex<Vec<Value>>);
impl GeneralEndpoint for Stub {
    fn model(&self) -> &str {
        "dream-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.0
            .lock()
            .unwrap()
            .pop()
            .ok_or(GeneralEndpointError::InvalidResponse("no queued response"))
    }
}
fn memory(id: u8, title: &str, content: &str) -> DreamMemory {
    DreamMemory {
        id: MemoryId([id; 32]),
        body_id: MemoryBodyId([id; 32]),
        category: "fact".into(),
        memory_type: "other".into(),
        lifecycle_state: "knowledge".into(),
        title: title.into(),
        content: content.into(),
        source_timestamp_ns: None,
        temporal: json!({"anchors":[],"patterns":[]}),
        graph_relations: vec![],
    }
}

#[test]
fn classifier_canonicalizes_pair_and_checks_verbatim_evidence() {
    let classifier = DreamClassifier::new(Stub(Mutex::new(vec![
        json!({"relation":"factual","direction":"a_to_b","evidence":[{"side":"a","quote":"Policy"},{"side":"b","quote":"Uses policy"}]}),
    ])));
    let a = memory(1, "Policy", "Defines retention.");
    let b = memory(2, "Runtime", "Uses policy during cleanup.");
    let result = classifier.classify_pair(&b, &a).unwrap();
    assert_eq!((result.a, result.b), (a.id, b.id));
    assert_eq!(result.direction, DreamRelationDirection::AToB);
    let bad = DreamClassifier::new(Stub(Mutex::new(vec![
        json!({"relation":"topical","direction":"undirected","evidence":[{"side":"a","quote":"fabricated"},{"side":"b","quote":"Uses policy"}]}),
    ])));
    assert!(bad.classify_pair(&a, &b).is_err());
}

#[test]
fn verifier_rejects_no_and_uncertain_signals_and_skips_unrequested_relations() {
    let a = memory(1, "Policy", "Defines retention.");
    let b = memory(2, "Runtime", "Uses policy during cleanup.");
    let classification = DreamPairClassification {
        model: "classifier".into(),
        a: a.id,
        b: b.id,
        relation: DreamRelationKind::Factual,
        direction: DreamRelationDirection::AToB,
        evidence: vec![],
    };
    let verifier = DreamVerifier::new(Stub(Mutex::new(vec![
        json!({"relation_supported":"yes","direction_supported":"uncertain","evidence_supported":"yes"}),
    ])));
    assert!(
        verifier
            .verify_if_required(DreamVerificationPolicy::default(), &classification, &a, &b)
            .unwrap()
            .is_none()
    );
    let result = verifier.verify_pair(&classification, &a, &b).unwrap();
    assert_eq!(result.verdict, DreamVerificationVerdict::Uncertain);
    assert_eq!(dream_verifier_schema()["additionalProperties"], false);
}
