use reliquary_chronos::TemporalAnalysis;
use reliquary_dream::{
    DreamCandidateContexts, DreamClassifier, DreamContext, DreamPairEvaluator,
    DreamRelationContext, DreamRelationKind, DreamVerificationPolicy, DreamVerificationVerdict,
    DreamVerifier,
};
use reliquary_memory_core::{Memory, MemoryBodyId, MemoryId};
use reliquary_switchboard::SimulatedGeneralEndpoint;
use serde_json::json;
fn memory(title: &str, content: &str) -> Memory {
    Memory {
        id: MemoryId([0; 32]),
        revision: 0,
        category: "note".into(),
        memory_type: "fact".into(),
        authority_kind: String::new(),
        temporal_status: String::new(),
        title: title.into(),
        content: content.into(),
        scope: String::new(),
        lifecycle_state: String::new(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        source_ref: None,
        temporal_inference: None,
        routing_metadata: None,
        mutation_id: String::new(),
        created_at_ns: 0,
        updated_at_ns: 0,
        global_version: 0,
        memory_version: 0,
    }
}

struct Context {
    memory: Memory,
    temporal: TemporalAnalysis,
}
impl DreamContext for Context {
    fn memory(&self) -> &Memory {
        &self.memory
    }
    fn body_id(&self) -> MemoryBodyId {
        MemoryBodyId([0; 32])
    }
    fn source_timestamp_ns(&self) -> Option<i64> {
        None
    }
    fn temporal(&self) -> &TemporalAnalysis {
        &self.temporal
    }
    fn relations(&self) -> Vec<DreamRelationContext> {
        vec![]
    }
}
#[test]
fn classify_and_verify_owned_inputs() {
    let mut a = Context {
        memory: memory("Alpha", "first"),
        temporal: TemporalAnalysis::default(),
    };
    let mut b = Context {
        memory: memory("Beta", "second"),
        temporal: TemporalAnalysis::default(),
    };
    a.memory.id = MemoryId([1; 32]);
    b.memory.id = MemoryId([2; 32]);
    let classifier = DreamClassifier::new(SimulatedGeneralEndpoint::new(
        "classifier",
        vec![
            json!({"relation":"duplicate_of", "direction":"undirected", "evidence":[{"side":"a", "quote":"Alpha"}, {"side":"b", "quote":"Beta"}]}),
        ],
    ));
    let result = classifier.classify_pair(&b, &a).unwrap();
    assert_eq!(result.relation, DreamRelationKind::DuplicateOf);
    assert_eq!(result.a, a.memory.id);
    let verifier = DreamVerifier::new(SimulatedGeneralEndpoint::new(
        "verifier",
        vec![
            json!({"relation_supported":"yes", "direction_supported":"yes", "evidence_supported":"yes"}),
        ],
    ));
    assert_eq!(
        verifier.verify_pair(&result, &b, &a).unwrap().verdict,
        DreamVerificationVerdict::Accept
    );
}

struct Set {
    source: Context,
    candidates: Vec<Context>,
}
impl DreamCandidateContexts for Set {
    type Context = Context;
    fn source(&self) -> &Context {
        &self.source
    }
    fn candidates(&self) -> Vec<&Context> {
        self.candidates.iter().collect()
    }
}
#[test]
fn frontier_zero_concurrency_still_evaluates() {
    let mut a = memory("Alpha", "first");
    let mut b = memory("Beta", "second");
    a.id = MemoryId([1; 32]);
    b.id = MemoryId([2; 32]);
    let set = Set {
        source: Context {
            memory: a,
            temporal: TemporalAnalysis::default(),
        },
        candidates: vec![Context {
            memory: b,
            temporal: TemporalAnalysis::default(),
        }],
    };
    let classifier = SimulatedGeneralEndpoint::new(
        "classifier",
        vec![json!({"relation":"none", "direction":"none", "evidence":[]})],
    );
    let verifier = SimulatedGeneralEndpoint::new("verifier", vec![]);
    let evaluator = DreamPairEvaluator::new(classifier, verifier);
    let result = evaluator
        .evaluate_frontier(&[&set], DreamVerificationPolicy::default(), 0)
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].len(), 1);
    assert!(result[0][0].is_ok());
}
