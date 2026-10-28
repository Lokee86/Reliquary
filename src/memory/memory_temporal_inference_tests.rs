use crate::memory_model::memory_body_id;
use crate::{
    CHRONOS_INFERENCE_CONTRACT_VERSION, MemoryDraft, MemoryTemporalInference, Phylactery,
    TemporalInference, TemporalInferenceResolution,
};
use std::fs;
use std::path::PathBuf;

#[test]
fn temporal_inference_round_trips_and_survives_matching_lifecycle_revisions() {
    let path = test_path("temporal.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let draft = draft("Review biweekly.", 100, "temporal-1");
    let body_id = memory_body_id(&draft.title, &draft.content);
    let inference = bound_inference(body_id, Some(100));
    let (memory, created) = phy
        .memories
        .publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            None,
            0,
            draft,
            None,
            Some(inference.clone()),
        )
        .unwrap();
    assert!(created);
    phy.sync().unwrap();
    drop(phy);

    let mut phy = Phylactery::open(&path).unwrap();
    let reopened = phy.memory(memory.id).unwrap();
    assert_eq!(reopened.temporal_inference, Some(inference.clone()));
    let analysis = phy.dream_temporal_analysis(memory.id).unwrap();
    assert_eq!(analysis.patterns.len(), 1);
    assert_eq!(analysis.patterns[0].interval, 2);
    assert_eq!(analysis.patterns[0].evidence, "biweekly");

    let (updated, changed) = phy
        .set_memory_temporal_status(
            memory.id,
            reopened.revision,
            "historical",
            "temporal-2".into(),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(updated.temporal_inference, Some(inference));
}

#[test]
fn explicit_temporal_inference_is_part_of_mutation_idempotency() {
    let path = test_path("idempotency.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let draft = draft("Review biweekly.", 100, "same-mutation");
    let body_id = memory_body_id(&draft.title, &draft.content);
    let first = bound_inference(body_id, Some(100));
    phy.memories
        .publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            None,
            0,
            draft.clone(),
            None,
            Some(first.clone()),
        )
        .unwrap();

    let mut conflicting = first;
    conflicting.inference.model = "different-model".into();
    assert!(matches!(
        phy.memories.publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            None,
            0,
            draft,
            None,
            Some(conflicting),
        ),
        Err(crate::MemoryError::MutationConflict)
    ));
}

#[test]
fn temporal_inference_is_dropped_when_reference_time_binding_changes() {
    let path = test_path("stale.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let initial = draft("Review biweekly.", 100, "stale-1");
    let body_id = memory_body_id(&initial.title, &initial.content);
    let (memory, _) = phy
        .memories
        .publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            None,
            0,
            initial,
            None,
            Some(bound_inference(body_id, Some(100))),
        )
        .unwrap();

    let mut changed = draft("Review biweekly.", 200, "stale-2");
    changed.lifecycle_state = "knowledge".into();
    let (updated, created) = phy
        .memories
        .publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            Some(memory.id),
            memory.revision,
            changed,
            None,
            None,
        )
        .unwrap();
    assert!(created);
    assert!(updated.temporal_inference.is_none());
}

fn bound_inference(
    body_id: crate::MemoryBodyId,
    source_time_ns: Option<i64>,
) -> MemoryTemporalInference {
    MemoryTemporalInference {
        body_id,
        source_time_ns,
        inference: crate::memory_temporal_conversion::to_memory_inference(TemporalInference {
            model: "test-model".into(),
            contract_version: CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 16,
                end_byte: 24,
                kind: crate::TemporalIndicationKind::Recurrence,
                evidence: "biweekly".into(),
                canonical_expression: "every two weeks".into(),
            }],
        }),
    }
}

fn draft(content: &str, source_time_ns: i64, mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Schedule".into(),
        content: content.into(),
        scope: "test".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(source_time_ns),
        mutation_id: mutation_id.into(),
        created_at_ns: 1,
        updated_at_ns: 2,
    }
}

fn test_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reliquary-temporal-inference-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}
