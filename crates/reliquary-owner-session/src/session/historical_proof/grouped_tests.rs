use super::*;
use reliquary_memory_core::{MemoryBodyId, MemoryRecord, memory_id};

fn record(version: u64, mutation: &str) -> Vec<u8> {
    reliquary_memory_core::encode_grouped_creation(&MemoryRecord {
        id: memory_id(mutation),
        revision: 1,
        body_id: MemoryBodyId([version as u8; 32]),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "owner".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "active".into(),
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
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        memory_version: version,
    })
    .unwrap()
}

#[test]
fn grouped_memory_publication_has_no_queryable_interior_cut() {
    let completion = GroupedInsomniaCompletion {
        episode_id: [3; 32],
        attempt: 1,
        started_at_ns: 1,
        completed_at_ns: 2,
        transaction_time_ns: 2,
        extractor_model: "fixture".into(),
        extractor_version: "1".into(),
        rejected_count: 0,
        memory_ids: vec![],
        external_memory_refs: vec![],
        global_version_start: 7,
        bodies: vec![],
        memory_records: vec![record(7, "first"), record(8, "second")],
        routing_metadata: vec![],
        freshness_birth_turn: None,
        freshness_policy_version: None,
        freshness_birth_memory_ids: vec![],
    };
    let completions = [((WorkEpisodeId([3; 32]), 1), completion)]
        .into_iter()
        .collect();
    assert!(validate_grouped_memory_cut(&completions, 6).is_ok());
    assert!(matches!(
        validate_grouped_memory_cut(&completions, 7),
        Err(HistoricalProofError::InconsistentCut(_))
    ));
    assert!(validate_grouped_memory_cut(&completions, 8).is_ok());
}
