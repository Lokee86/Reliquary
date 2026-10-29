use super::*;
use crate::{DreamRelationContext, dream_classifier_schema};
use reliquary_chronos::TemporalAnalysis;
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
fn rejects_nonverbatim_evidence_and_preserves_canonical_pair_order() {
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
    let endpoint = SimulatedGeneralEndpoint::new(
        "test",
        vec![
            json!({"relation":"topical", "direction":"undirected", "evidence":[{"side":"a", "quote":"invented"},{"side":"b", "quote":"Beta"}]}),
        ],
    );
    assert!(matches!(
        DreamClassifier::new(endpoint).classify_pair(&b, &a),
        Err(DreamClassificationError::InvalidOutput(_))
    ));
    let endpoint = SimulatedGeneralEndpoint::new(
        "test",
        vec![
            json!({"relation":"topical", "direction":"undirected", "evidence":[{"side":"a", "quote":"Alpha"},{"side":"b", "quote":"Beta"}]}),
        ],
    );
    let result = DreamClassifier::new(endpoint)
        .classify_pair(&b, &a)
        .unwrap();
    assert_eq!((result.a, result.b), (a.memory.id, b.memory.id));
    assert_eq!(dream_classifier_schema()["additionalProperties"], false);
}
