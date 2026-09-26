use crate::dream_cooldown::DREAM_PROCESSING_LANE;
use crate::memory_model::memory_body_id;
use crate::{
    CHRONOS_INFERENCE_CONTRACT_VERSION, Container, Cva, EchoEvent, EchoEventKind, EntityDraft,
    FragmentConfig, GraphRelationKind, MemoryDraft, MemoryId, MemoryTemporalInference,
    MigrationError, Phylactery, ReliquaryScopeKind, SimulatedEmbeddingEndpoint,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution, VectorNormalization,
    migrate_file,
};
use std::fs;
use std::path::PathBuf;

fn test_dir() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-migration-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn draft(mutation: &str, memory_type: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: memory_type.into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: mutation.into(),
        content: content.into(),
        scope: "private".into(),
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
        source_time_ns: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn append_legacy_dream_v1(container: &mut Container, id: MemoryId, epoch: u64) {
    let mut payload = b"CVADREM1".to_vec();
    payload.extend_from_slice(&id.0);
    payload.extend_from_slice(&epoch.to_le_bytes());
    container.append(&payload).unwrap();
}

fn append_legacy_dream_v2(
    container: &mut Container,
    id: MemoryId,
    epoch: u64,
    processed_at_ns: i64,
) {
    let mut payload = b"CVADREM2".to_vec();
    payload.extend_from_slice(&id.0);
    payload.extend_from_slice(&epoch.to_le_bytes());
    payload.extend_from_slice(&processed_at_ns.to_le_bytes());
    container.append(&payload).unwrap();
}

fn append_legacy_workspace_id(cva: &mut Cva, id: &str) {
    let mut payload = b"CVAWKSP1".to_vec();
    for value in [id, "Old Project", "construction"] {
        payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
        payload.extend_from_slice(value.as_bytes());
    }
    cva.container.append(&payload).unwrap();
}

fn seed_archive(cva: &mut Cva) {
    for index in 0..8 {
        cva.append_node(
            format!("n{index}"),
            "c".into(),
            (index > 0).then(|| format!("n{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index,
            "migration archive text",
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        "c",
        "n7",
        FragmentConfig {
            turns: 4,
            overlap: 1,
        },
        true,
    )
    .unwrap();
}

#[test]
fn legacy_typed_rel_migration_preserves_identity_and_state() {
    let dir = test_dir();
    let source = dir.join("old.prj.rel");
    let copy = dir.join("old-copy.prj.rel");
    let output = dir.join("new.prj.rel");
    let copy_output = dir.join("new-copy.prj.rel");

    let mut rel = Cva::create_legacy_typed(&source, ReliquaryScopeKind::Project).unwrap();
    seed_archive(&mut rel);
    let (memory, _) = rel
        .publish_memory(None, 0, draft("project:m1", "project", "remember this"))
        .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 17);
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    rel.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    rel.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    rel.put_conversation_compaction(
        "conversation-old".into(),
        "n5".into(),
        "durable migrated summary".into(),
        None,
    )
    .unwrap();
    rel.put_echo_event(EchoEvent {
        conversation_id: "c".into(),
        message_id: "n1".into(),
        sequence: 0,
        timestamp_ns: 1,
        model_round: Some(1),
        kind: EchoEventKind::ReasoningTrace,
        correlation_id: None,
        name: None,
        content: "migrated reasoning".into(),
    })
    .unwrap();
    append_legacy_workspace_id(&mut rel, "workspace-shared");
    rel.sync().unwrap();
    drop(rel);
    fs::copy(&source, &copy).unwrap();

    let first = migrate_file(&source, &output).unwrap();
    let second = migrate_file(&copy, &copy_output).unwrap();
    assert!(first.derived_from_legacy_workspace_id);
    assert_eq!(first.owner_id, second.owner_id);
    assert!(first.owner_id.starts_with("rel-"));

    let mut migrated = Cva::open(&output).unwrap();
    assert_eq!(migrated.stats().nodes, 8);
    assert_eq!(migrated.memory(memory.id).unwrap().content, "remember this");
    assert_eq!(migrated.compatibility_profile_stats().profiles, 1);
    assert_eq!(migrated.memory_vector_stats().bindings, 1);
    assert!(migrated.archive_vector_stats().objects > 0);
    assert_eq!(migrated.vector_generation_stats().generations, 1);
    assert!(migrated.current_vector_generation(profile.id).is_some());
    let compactions = migrated.conversation_compactions("conversation-old");
    assert_eq!(compactions.len(), 1);
    assert_eq!(compactions[0].through_message_id, "n5");
    assert_eq!(compactions[0].summary, "durable migrated summary");
    let echo = migrated.echo_events("c", "n1");
    assert_eq!(echo.len(), 1);
    assert_eq!(echo[0].kind, EchoEventKind::ReasoningTrace);
    assert_eq!(echo[0].content, "migrated reasoning");
}

#[test]
fn legacy_typed_phy_migration_preserves_owned_state() {
    let dir = test_dir();
    let source = dir.join("old.phy");
    let output = dir.join("new.phy");
    let mut phy = Phylactery::create_legacy_typed(&source).unwrap();
    let a_draft = draft("user:a", "user", "prefers Helix; review biweekly");
    let a_body_id = memory_body_id(&a_draft.title, &a_draft.content);
    let temporal_inference = MemoryTemporalInference {
        body_id: a_body_id,
        source_time_ns: a_draft.source_time_ns,
        inference: TemporalInference {
            model: "chronos-test".into(),
            contract_version: CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 22,
                end_byte: 30,
                kind: TemporalIndicationKind::Recurrence,
                evidence: "biweekly".into(),
                canonical_expression: "every two weeks".into(),
            }],
        },
    };
    let (a, _) = phy
        .memories
        .publish_with_source_ref_and_temporal_inference(
            &mut phy.container,
            None,
            0,
            a_draft,
            None,
            Some(temporal_inference.clone()),
        )
        .unwrap();
    let (b, _) = phy
        .publish_memory(None, 0, draft("user:b", "user", "prefers Nushell"))
        .unwrap();
    phy.set_memory_relation(a.id, b.id, GraphRelationKind::Topical, true, 0)
        .unwrap();
    let (entity, _) = phy
        .publish_entity(
            None,
            0,
            EntityDraft {
                canonical_name: "Helix".into(),
                aliases: vec![],
                kind: "project".into(),
                summary: "User project referent.".into(),
                mutation_id: "user:helix-entity".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    phy.set_entity_association(a.id, entity.id, true, 1)
        .unwrap();
    phy.mark_dream_processed(a.id, 2, 123).unwrap();
    phy.mark_dream_pair_evaluated(a.id, b.id).unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 23);
    let profile = phy.establish_compatibility_profile(&endpoint).unwrap();
    phy.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    phy.sync().unwrap();
    drop(phy);
    let result = migrate_file(&source, &output).unwrap();
    assert!(!result.derived_from_legacy_workspace_id);
    assert!(result.owner_id.starts_with("phy-"));
    let mut migrated = Phylactery::open(&output).unwrap();
    let migrated_a = migrated.memory(a.id).unwrap();
    assert_eq!(migrated_a.content, "prefers Helix; review biweekly");
    assert_eq!(migrated_a.temporal_inference, Some(temporal_inference));
    assert_eq!(
        migrated
            .dream_temporal_analysis(a.id)
            .unwrap()
            .patterns
            .len(),
        1
    );
    assert_eq!(migrated.memory_stats().memories, 2);
    assert_eq!(migrated.graph_stats().active_relations, 2);
    assert_eq!(migrated.graph_stats().memory_active_relations, 1);
    assert_eq!(migrated.entity(entity.id).unwrap().canonical_name, "Helix");
    assert_eq!(
        migrated.entity(entity.id).unwrap().global_id,
        entity.global_id
    );
    assert_eq!(
        migrated.entity_associations_for_memory(a.id),
        vec![entity.id]
    );
    assert_eq!(migrated.compatibility_profile_stats().profiles, 1);
    assert_eq!(migrated.memory_vector_stats().bindings, 2);
    assert_eq!(
        migrated
            .dream_cooldown_records()
            .into_iter()
            .find(|(id, _)| *id == a.id)
            .map(|(_, state)| (state.epoch, state.processed_at_ns)),
        Some((2, Some(123)))
    );
    assert_eq!(migrated.dream_pair_records().len(), 1);
}

#[test]
fn legacy_rel_migration_preserves_dream_maintenance_state() {
    let dir = test_dir();
    let source = dir.join("old-dream.prj.rel");
    let output = dir.join("new-dream.rel");
    let mut rel = Cva::create_legacy_typed(&source, ReliquaryScopeKind::Project).unwrap();
    let (a, _) = rel
        .publish_memory(None, 0, draft("dream:a", "project", "A"))
        .unwrap();
    let (b, _) = rel
        .publish_memory(None, 0, draft("dream:b", "project", "B"))
        .unwrap();
    append_legacy_dream_v1(&mut rel.container, a.id, 3);
    append_legacy_dream_v2(&mut rel.container, b.id, 2, 456);
    rel.mark_dream_pair_evaluated(a.id, b.id).unwrap();
    rel.sync().unwrap();
    drop(rel);

    migrate_file(&source, &output).unwrap();
    let migrated = Cva::open(&output).unwrap();
    assert_eq!(
        migrated
            .dream_cooldown_records()
            .into_iter()
            .find(|(id, _)| *id == a.id)
            .map(|(_, state)| (state.epoch, state.processed_at_ns)),
        Some((3, Some(1)))
    );
    assert_eq!(
        migrated
            .dream_cooldown_records()
            .into_iter()
            .find(|(id, _)| *id == b.id)
            .map(|(_, state)| (state.epoch, state.processed_at_ns)),
        Some((2, Some(456)))
    );
    assert_eq!(
        migrated
            .processing_epochs
            .state(a.id, DREAM_PROCESSING_LANE)
            .unwrap()
            .cadence_version,
        1
    );
    assert_eq!(migrated.dream_pair_records().len(), 1);
}

#[test]
fn legacy_cva_migrates_once_to_current_project_rel() {
    let dir = test_dir();
    let source = dir.join("old.cva");
    let output = dir.join("new.prj.rel");
    let retry_output = dir.join("retry.prj.rel");
    let mut legacy = Cva::create_legacy_cva(&source).unwrap();
    legacy
        .append_node("n".into(), "c".into(), None, "user".into(), 1, "legacy")
        .unwrap();
    legacy.sync().unwrap();
    drop(legacy);

    let result = migrate_file(&source, &output).unwrap();
    assert!(!result.derived_from_legacy_workspace_id);
    assert_eq!(result.scope, Some(ReliquaryScopeKind::Project));
    assert!(result.owner_id.starts_with("rel-"));
    let migrated = Cva::open(&output).unwrap();
    assert_eq!(migrated.stats().nodes, 1);
    assert!(migrated.conversation_compactions("c").is_empty());

    assert!(matches!(
        migrate_file(&output, &retry_output),
        Err(MigrationError::AlreadyCurrent)
    ));
    assert!(!retry_output.exists());
}
