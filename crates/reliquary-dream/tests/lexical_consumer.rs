use reliquary_dream::lexical_score;
use reliquary_memory_core::{Memory, MemoryId};

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

#[test]
fn lexical_policy_scores_tokens_and_metadata() {
    let mut a = memory("Blue sky", "clouds");
    let mut b = memory("blue ocean", "clouds");
    assert!((lexical_score(&a, &b) - (0.5 + 0.16)).abs() < 1e-12);
    a.parent_id = Some(MemoryId([1; 32]));
    b.parent_id = a.parent_id;
    assert!((lexical_score(&a, &b) - (0.5 + 0.28)).abs() < 1e-12);
}
