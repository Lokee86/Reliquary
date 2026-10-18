use crate::imports::{ImportBranch, ImportConversation, ImportMessage, NativeImport};
use crate::{Cva, EntityDraft, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::fs;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-characterization-{}-{name}",
        uuid::Uuid::new_v4()
    ))
}

fn entity(name: &str, mutation: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![],
        kind: "test".into(),
        summary: "characterization".into(),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn native_input() -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: "characterization".into(),
        source_format: "structured-agent-v1".into(),
        conversations: vec![ImportConversation {
            id: "conversation".into(),
            title: Some("Characterization".into()),
            messages: vec![
                ImportMessage {
                    id: "user".into(),
                    revision: Some("1".into()),
                    parent_id: None,
                    replaces: None,
                    deleted: false,
                    role: "user".into(),
                    timestamp_ns: 1,
                    content: "question".into(),
                    artifacts: vec![],
                },
                ImportMessage {
                    id: "assistant".into(),
                    revision: Some("1".into()),
                    parent_id: Some("user".into()),
                    replaces: None,
                    deleted: false,
                    role: "assistant".into(),
                    timestamp_ns: 2,
                    content: "answer".into(),
                    artifacts: vec![],
                },
            ],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "assistant".into(),
                canonical: true,
            }],
        }],
    }
}

#[test]
fn current_entity_merge_tombstone_survives_reopen_and_keeps_survivor_live() {
    let file = path("entity.rel");
    let mut rel = Cva::create(&file).unwrap();
    let survivor = rel
        .publish_entity(None, 0, entity("survivor", "survivor"))
        .unwrap()
        .0;
    let retired = rel
        .publish_entity(None, 0, entity("retired", "retired"))
        .unwrap()
        .0;
    let outcome = rel.merge_entities(survivor.id, retired.id, 10).unwrap();
    assert_eq!(outcome.survivor_id, survivor.id);
    assert_eq!(rel.entity(survivor.id).unwrap().canonical_name, "survivor");
    assert!(matches!(
        rel.entity(retired.id),
        Err(crate::EntityError::MissingEntity)
    ));
    assert_eq!(
        rel.retired_entity_replacement(retired.id),
        Some(survivor.id)
    );
    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&file).unwrap();
    assert_eq!(
        reopened.entity(survivor.id).unwrap().canonical_name,
        "survivor"
    );
    assert!(matches!(
        reopened.entity(retired.id),
        Err(crate::EntityError::MissingEntity)
    ));
    assert_eq!(
        reopened.retired_entity_replacement(retired.id),
        Some(survivor.id)
    );
    let _ = fs::remove_file(file);
}

#[test]
fn archive_activity_versions_and_positions_survive_reopen() {
    let file = path("activity.rel");
    let mut rel = Cva::create(&file).unwrap();
    rel.append_node("u".into(), "c".into(), None, "user".into(), 1, "question")
        .unwrap();
    rel.append_node(
        "a".into(),
        "c".into(),
        Some("u".into()),
        "assistant".into(),
        2,
        "answer",
    )
    .unwrap();
    let version = rel.archive_version();
    assert_eq!(rel.activity_position_for_turn("c", "u"), Some(1));
    assert_eq!(rel.activity_position_for_turn("c", "a"), Some(2));
    assert_eq!(rel.activity_cut_at_archive_version(version).unwrap(), 2);
    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.archive_version(), version);
    assert_eq!(reopened.activity_position_for_turn("c", "u"), Some(1));
    assert_eq!(reopened.activity_position_for_turn("c", "a"), Some(2));
    let _ = fs::remove_file(file);
}

#[test]
fn repeated_native_import_is_idempotent_after_reopen() {
    let file = path("import.rel");
    let mut rel = Cva::create(&file).unwrap();
    let input = native_input();
    let receipt = rel.import_native(&input, 100).unwrap();
    let versions = (rel.archive_version(), rel.latest_global_version());
    rel.sync().unwrap();
    drop(rel);
    let mut reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.import_native(&input, 200).unwrap(), receipt);
    assert_eq!(
        (reopened.archive_version(), reopened.latest_global_version()),
        versions
    );
    assert_eq!(reopened.native_import_receipts(), vec![receipt.clone()]);
    assert!(
        !fs::read(&file)
            .unwrap()
            .windows(8)
            .any(|w| w == b"CVAFRS03")
    );
    let mut later = native_input();
    later.conversations[0].id = "later-conversation".into();
    later.conversations[0].messages[0].content = "later question".into();
    assert_ne!(reopened.import_native(&later, 300).unwrap(), receipt);
    assert_eq!(reopened.native_import_receipts().len(), 2);
}

#[test]
fn reclaim_and_profile_generation_reopen_preserve_current_high_level_state() {
    let file = path("profile.rel");
    let output = path("profile-reclaimed.rel");
    let mut rel = Cva::create(&file).unwrap();
    rel.append_node(
        "u".into(),
        "c".into(),
        None,
        "user".into(),
        1,
        "profile source",
    )
    .unwrap();
    rel.materialize_path_fragments("c", "u", crate::FragmentConfig::default(), true)
        .unwrap();
    assert!(!rel.fragments().is_empty());
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    rel.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    rel.sync().unwrap();
    rel.reclaim_storage(&output).unwrap();
    let reopened = Cva::open(&output).unwrap();
    assert_eq!(
        reopened.compatibility_profile(profile.id).unwrap(),
        rel.compatibility_profile(profile.id).unwrap()
    );
    assert!(reopened.current_vector_generation(profile.id).is_some());
    let _ = fs::remove_file(file);
    let _ = fs::remove_file(output);
}
