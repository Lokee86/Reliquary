use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct SequenceEndpoint {
    responses: Mutex<Vec<Result<Value, GeneralEndpointError>>>,
    calls: Arc<AtomicUsize>,
}

impl SequenceEndpoint {
    fn new(responses: Vec<Result<Value, GeneralEndpointError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl GeneralEndpoint for SequenceEndpoint {
    fn model(&self) -> &str {
        "dream-retry-test"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or(Err(GeneralEndpointError::InvalidResponse(
                "empty response queue",
            )))
    }
}

fn memory(id: u8) -> DreamMemory {
    DreamMemory {
        id: MemoryId([id; 32]),
        body_id: MemoryBodyId([id; 32]),
        category: "fact".into(),
        memory_type: "other".into(),
        lifecycle_state: "knowledge".into(),
        title: format!("Memory {id}"),
        content: format!("Evidence for memory {id}."),
        source_timestamp_ns: None,
        temporal: json!({"anchors":[],"patterns":[]}),
        graph_relations: vec![],
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

fn none_response() -> Value {
    json!({"relation":"none","direction":"none","evidence":[]})
}
fn no_verification_endpoint() -> SequenceEndpoint {
    SequenceEndpoint::new(vec![])
}

#[test]
fn pair_evaluator_retries_invalid_output_but_not_backpressure() {
    let classifier = SequenceEndpoint::new(vec![
        Ok(none_response()),
        Ok(json!({"relation":"unknown","direction":"none","evidence":[]})),
    ]);
    let classifier_calls = classifier.calls.clone();
    let evaluator = DreamPairEvaluator::new(classifier, no_verification_endpoint());
    let result = evaluator
        .evaluate_pair_with_retries(DreamVerificationPolicy::default(), &memory(1), &memory(2))
        .unwrap();
    assert_eq!(result.0.relation, DreamRelationKind::None);
    assert_eq!(classifier_calls.load(Ordering::SeqCst), 2);

    let busy = SequenceEndpoint::new(vec![Err(GeneralEndpointError::Backpressure {
        message: "busy".into(),
        retry_after: None,
    })]);
    let busy_calls = busy.calls.clone();
    let evaluator = DreamPairEvaluator::new(busy, no_verification_endpoint());
    let error = evaluator
        .evaluate_pair_with_retries(DreamVerificationPolicy::default(), &memory(1), &memory(2))
        .unwrap_err();
    assert!(error.is_backpressure());
    assert_eq!(busy_calls.load(Ordering::SeqCst), 1);

    let exhausted = SequenceEndpoint::new(vec![
        Ok(json!({"relation":"bad","direction":"none","evidence":[]})),
        Ok(json!({"relation":"bad","direction":"none","evidence":[]})),
        Ok(json!({"relation":"bad","direction":"none","evidence":[]})),
    ]);
    let exhausted_calls = exhausted.calls.clone();
    let evaluator = DreamPairEvaluator::new(exhausted, no_verification_endpoint());
    assert!(
        evaluator
            .evaluate_pair_with_retries(DreamVerificationPolicy::default(), &memory(1), &memory(2))
            .is_err()
    );
    assert_eq!(exhausted_calls.load(Ordering::SeqCst), 3);
}

#[test]
fn verifier_failures_retry_the_full_pair_and_backpressure_stops() {
    let classification = json!({"relation":"factual","direction":"a_to_b","evidence":[{"side":"a","quote":"Memory 1"},{"side":"b","quote":"Evidence for memory 2."}]});
    let classifier = SequenceEndpoint::new(vec![Ok(classification.clone()), Ok(classification)]);
    let classifier_calls = classifier.calls.clone();
    let verifier = SequenceEndpoint::new(vec![
        Ok(
            json!({"relation_supported":"yes","direction_supported":"yes","evidence_supported":"yes"}),
        ),
        Ok(
            json!({"relation_supported":"maybe","direction_supported":"yes","evidence_supported":"yes"}),
        ),
    ]);
    let verifier_calls = verifier.calls.clone();
    let evaluator = DreamPairEvaluator::new(classifier, verifier);
    let result = evaluator
        .evaluate_pair_with_retries(
            DreamVerificationPolicy::broad_semantic(),
            &memory(1),
            &memory(2),
        )
        .unwrap();
    assert_eq!(result.0.relation, DreamRelationKind::Factual);
    assert_eq!(result.1.unwrap().verdict, DreamVerificationVerdict::Accept);
    assert_eq!(classifier_calls.load(Ordering::SeqCst), 2);
    assert_eq!(verifier_calls.load(Ordering::SeqCst), 2);

    let classifier = SequenceEndpoint::new(vec![Ok(json!({
        "relation":"factual","direction":"a_to_b","evidence":[{"side":"a","quote":"Memory 1"},{"side":"b","quote":"Evidence for memory 2."}]
    }))]);
    let verifier = SequenceEndpoint::new(vec![Err(GeneralEndpointError::Backpressure {
        message: "busy".into(),
        retry_after: None,
    })]);
    let verifier_calls = verifier.calls.clone();
    let evaluator = DreamPairEvaluator::new(classifier, verifier);
    let error = evaluator
        .evaluate_pair_with_retries(
            DreamVerificationPolicy::broad_semantic(),
            &memory(1),
            &memory(2),
        )
        .unwrap_err();
    assert!(error.is_backpressure());
    assert_eq!(verifier_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn candidate_evaluation_respects_pair_concurrency_and_order() {
    struct ConcurrentEndpoint(Arc<(AtomicUsize, AtomicUsize)>);
    impl GeneralEndpoint for ConcurrentEndpoint {
        fn model(&self) -> &str {
            "dream-concurrency-test"
        }
        fn complete_json(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            let (active, peak) = &*self.0;
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(none_response())
        }
    }
    let counts = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
    let evaluator = DreamPairEvaluator::new(
        ConcurrentEndpoint(counts.clone()),
        no_verification_endpoint(),
    );
    let set = CandidateSet {
        source: memory(1),
        candidates: (2..=6).map(memory).collect(),
    };
    let output = evaluator
        .evaluate_candidates(&set, DreamVerificationPolicy::default(), 2)
        .unwrap();
    assert_eq!(output.len(), 5);
    for (index, (classification, verification)) in output.iter().enumerate() {
        assert_eq!(classification.a, MemoryId([1; 32]));
        assert_eq!(classification.b, MemoryId([(index + 2) as u8; 32]));
        assert_eq!(classification.relation, DreamRelationKind::None);
        assert!(verification.is_none());
    }
    assert!(counts.1.load(Ordering::SeqCst) <= 2);
}

#[test]
fn frontier_returns_backpressure_without_retrying_the_blocked_pair() {
    let endpoint = SequenceEndpoint::new(vec![Err(GeneralEndpointError::Backpressure {
        message: "busy".into(),
        retry_after: None,
    })]);
    let calls = endpoint.calls.clone();
    let evaluator = DreamPairEvaluator::new(endpoint, no_verification_endpoint());
    let first = CandidateSet {
        source: memory(1),
        candidates: vec![memory(2)],
    };
    let second = CandidateSet {
        source: memory(3),
        candidates: vec![memory(4)],
    };
    let staged = [&first, &second];
    let error = evaluator
        .evaluate_frontier(&staged, DreamVerificationPolicy::default(), 1)
        .unwrap_err();
    assert!(error.is_backpressure());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
