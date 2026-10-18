use crate::imports::{ImportBranch, ImportConversation, ImportMessage, NativeImport};
use crate::owner_lifecycle::{LifecycleTarget, PayloadUnavailable};
use crate::{ArchiveError, Cva, HistoryPruneRequest};
use crate::{FragmentConfig, SimulatedEmbeddingEndpoint, VectorNormalization};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn paths() -> (PathBuf, PathBuf) {
    let dir = Path::new(".test-tmp").join(format!("history-prune-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    (dir.join("source.rel"), dir.join("pruned.rel"))
}

fn request(ids: &[&str]) -> HistoryPruneRequest {
    HistoryPruneRequest {
        targets: ids
            .iter()
            .map(|id| LifecycleTarget::Conversation((*id).to_owned()))
            .collect(),
    }
}

fn append(cva: &mut Cva, conversation: &str, node: &str, body: &str) {
    cva.append_node(
        node.to_owned(),
        conversation.to_owned(),
        None,
        "user".into(),
        42,
        body,
    )
    .unwrap();
}

fn import_input() -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: "history-prune-test".into(),
        source_format: "history-prune-v1".into(),
        conversations: vec![ImportConversation {
            id: "imported".into(),
            title: None,
            messages: vec![ImportMessage {
                id: "node".into(),
                revision: Some("1".into()),
                parent_id: None,
                replaces: None,
                deleted: false,
                role: "user".into(),
                timestamp_ns: 42,
                content: "settled import payload".repeat(2500),
                artifacts: vec![],
            }],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "node".into(),
                canonical: true,
            }],
        }],
    }
}

#[test]
fn explicit_prune_removes_only_tombstoned_body_and_preserves_structure() {
    let (source, output) = paths();
    let private = "private historical body ".repeat(4000);
    let mut cva = Cva::create_project(&source).unwrap();
    append(&mut cva, "retired", "old", &private);
    append(&mut cva, "live", "current", "still available");
    cva.tombstone_conversation("retired").unwrap();
    cva.sync().unwrap();
    let source_bytes = fs::read(&source).unwrap();
    let tombstone = cva.conversation_tombstone("retired").unwrap().clone();
    let before = (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.entity_version(),
        cva.relationship_version(),
        cva.graph_version(),
        cva.memory_graph_version(),
        cva.vector_version(),
    );

    let report = cva
        .prune_tombstoned_history(&output, request(&["retired"]))
        .unwrap();
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(report.pruned_content_objects, 1);
    assert!(report.output_file_bytes < report.source_file_bytes);
    assert!(
        !fs::read(&output)
            .unwrap()
            .windows(b"private historical body".len())
            .any(|window| window == b"private historical body")
    );

    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(
        (
            reopened.latest_global_version(),
            reopened.archive_version(),
            reopened.memory_version(),
            reopened.entity_version(),
            reopened.relationship_version(),
            reopened.graph_version(),
            reopened.memory_graph_version(),
            reopened.vector_version(),
        ),
        before
    );
    assert_eq!(reopened.conversation_tombstone("retired"), Some(&tombstone));
    assert_eq!(
        reopened.conversation_turns("live", "current").unwrap()[0].content,
        "still available"
    );
    assert!(matches!(
        reopened.historical_conversation_turns("retired", "old"),
        Err(ArchiveError::PayloadUnavailable(PayloadUnavailable::Pruned))
    ));
    assert!(matches!(
        reopened.conversation_turns("retired", "old"),
        Err(ArchiveError::TombstonedConversation)
    ));
    drop(reopened);
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn shared_live_body_and_nonterminal_targets_are_rejected_before_output() {
    let (source, output) = paths();
    let body = "shared private content ".repeat(3000);
    let mut cva = Cva::create(&source).unwrap();
    append(&mut cva, "retired", "old", &body);
    append(&mut cva, "live", "new", &body);
    cva.tombstone_conversation("retired").unwrap();
    let source_bytes = fs::read(&source).unwrap();
    assert!(
        cva.prune_tombstoned_history(&output, request(&["retired"]))
            .is_err()
    );
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert!(
        cva.prune_tombstoned_history(&output, request(&["live"]))
            .is_err()
    );
    assert!(!output.exists());
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn settled_import_prefix_content_is_blocked_but_later_native_body_is_prunable() {
    let (source, output) = paths();
    let mut cva = Cva::create_project(&source).unwrap();
    cva.import_native(&import_input(), 100).unwrap();
    crate::imports::tests::finish_current_import(&mut cva, 110);

    // The imported node's content is inside the authenticated source prefix.
    let imported_conversation = cva
        .archive
        .nodes
        .iter()
        .next()
        .unwrap()
        .conversation_id
        .clone();
    cva.tombstone_conversation(&imported_conversation).unwrap();
    assert!(
        cva.prune_tombstoned_history(&output, request(&[&imported_conversation]))
            .is_err()
    );
    assert!(!output.exists());

    // A later, unrelated tombstoned Archive body is outside every settled
    // source prefix and can be removed while the original import authority stays fixed.
    append(
        &mut cva,
        "later-native",
        "old",
        &"later private body ".repeat(3000),
    );
    cva.tombstone_conversation("later-native").unwrap();
    cva.prune_tombstoned_history(&output, request(&["later-native"]))
        .unwrap();
    let reopened = Cva::open(&output).unwrap();
    assert_eq!(reopened.native_import_receipts().len(), 1);
    drop(reopened);
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn retired_profile_prune_keeps_vector_topology_and_marks_matrix_unavailable() {
    let (source, output) = paths();
    let mut cva = Cva::create(&source).unwrap();
    for index in 0..6 {
        cva.append_node(
            format!("node-{index}"),
            "vector-conversation".into(),
            (index > 0).then(|| format!("node-{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            &format!("vector-prune fixture turn {index}"),
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        "vector-conversation",
        "node-5",
        FragmentConfig::default(),
        true,
    )
    .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(512, VectorNormalization::L2, 91);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap();
    let generation = cva
        .build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let archive_set = cva.archive_vectors(generation.archive_vector_id).unwrap();
    let packed_id = archive_set.packed_vector_id;
    let packed_before = cva.packed_vectors(packed_id).unwrap();
    let packed_payload = packed_before.bytes().to_vec();
    let packed_len = cva
        .packed_vector_infos()
        .iter()
        .find(|info| info.id == packed_id)
        .unwrap()
        .byte_len;
    cva.retire_compatibility_profile(profile.id).unwrap();
    cva.sync().unwrap();

    let report = cva
        .prune_tombstoned_history(
            &output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::CompatibilityProfile(profile.id)],
            },
        )
        .unwrap();
    assert_eq!(
        fs::read(&source).unwrap().len() as u64,
        report.source_file_bytes
    );
    assert_eq!(report.pruned_packed_vectors, 1);
    assert_eq!(report.destroyed_packed_bytes, packed_len);
    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(
        reopened.historical_vector_generation(profile.id),
        Some(generation)
    );
    assert!(
        reopened
            .archive_vectors(generation.archive_vector_id)
            .is_ok()
    );
    assert!(matches!(
        reopened.packed_vectors(packed_id),
        Err(crate::PackedVectorError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(
        !fs::read(&output)
            .unwrap()
            .windows(packed_payload.len())
            .any(|window| window == packed_payload)
    );
    drop(reopened);
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

fn memory_draft(mutation: &str, title: &str, content: &str) -> crate::MemoryDraft {
    crate::MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: content.into(),
        scope: "user".into(),
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
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn memory_body_prune_retains_revision_topology_and_rejects_unbound_marker() {
    let (source, output) = paths();
    let mut cva = Cva::create_project(&source).unwrap();
    let secret = "private memory body ".repeat(4000);
    let memory = cva
        .publish_memory(None, 0, memory_draft("prune-memory", "Old", &secret))
        .unwrap()
        .0;
    cva.tombstone_memory(memory.id, memory.revision).unwrap();
    let descriptor = cva.memories.retention_descriptor(memory.id).unwrap();
    let clocks = (cva.latest_global_version(), cva.memory_version());
    cva.prune_tombstoned_history(
        &output,
        HistoryPruneRequest {
            targets: vec![LifecycleTarget::Memory(memory.id)],
        },
    )
    .unwrap();
    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(
        clocks,
        (reopened.latest_global_version(), reopened.memory_version())
    );
    assert_eq!(
        descriptor,
        reopened.memories.retention_descriptor(memory.id).unwrap()
    );
    assert!(matches!(
        reopened.memory_revision(memory.id, 1),
        Err(crate::MemoryError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(
        !fs::read(&output)
            .unwrap()
            .windows(secret.len())
            .any(|w| w == secret.as_bytes())
    );
    let forged = source.with_file_name("forged.rel");
    let mut bad = Cva::create_project(&forged).unwrap();
    let body = crate::MemoryBodyId([7; 32]);
    bad.container
        .append(&crate::memory_store::history_prune::PrunedMemoryBody::encode(body, 123, [8; 32]))
        .unwrap();
    bad.sync().unwrap();
    drop(bad);
    assert!(Cva::open(&forged).is_err());
    drop(reopened);
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn entity_relationship_descriptions_prune_with_topology_and_references_intact() {
    let (source, output) = paths();
    let mut cva = Cva::create_project(&source).unwrap();
    let secret = "private description ".repeat(160);
    let entity = cva
        .publish_entity(
            None,
            0,
            crate::EntityDraft {
                canonical_name: "Retired Person".into(),
                aliases: vec!["Old Alias".into()],
                kind: "person".into(),
                summary: secret.clone(),
                mutation_id: "prune-entity".into(),
                created_at_ns: 10,
                updated_at_ns: 10,
            },
        )
        .unwrap()
        .0;
    let relationship = cva
        .publish_relationship(
            None,
            0,
            crate::RelationshipDraft {
                kind: "association".into(),
                participants: vec![crate::RelationshipParticipant {
                    entity: crate::EntityRef {
                        owner_id: cva.owner_id().unwrap(),
                        entity_id: entity.id,
                    },
                    role: None,
                }],
                evidence: vec![],
                summary: secret.clone(),
                mutation_id: "prune-relationship".into(),
                created_at_ns: 11,
                updated_at_ns: 11,
            },
        )
        .unwrap()
        .0;
    cva.tombstone_entity(entity.id, entity.revision).unwrap();
    cva.tombstone_relationship(relationship.id, relationship.revision)
        .unwrap();
    let e = cva.entities.retention_descriptor(entity.id).unwrap();
    let r = cva
        .relationships
        .retention_descriptor(relationship.id)
        .unwrap();
    cva.prune_tombstoned_history(
        &output,
        HistoryPruneRequest {
            targets: vec![
                LifecycleTarget::Entity(entity.id),
                LifecycleTarget::Relationship(relationship.id),
            ],
        },
    )
    .unwrap();
    let reopened = Cva::open(&output).unwrap();
    assert_eq!(
        e,
        reopened.entities.retention_descriptor(entity.id).unwrap()
    );
    assert_eq!(
        r,
        reopened
            .relationships
            .retention_descriptor(relationship.id)
            .unwrap()
    );
    assert!(matches!(
        reopened.entities.entity_revision(entity.id, 1),
        Err(crate::EntityError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(matches!(
        reopened
            .relationships
            .relationship_revision(relationship.id, 1),
        Err(crate::RelationshipError::PayloadUnavailable(
            PayloadUnavailable::Pruned
        ))
    ));
    assert!(
        !fs::read(&output)
            .unwrap()
            .windows(secret.len())
            .any(|w| w == secret.as_bytes())
    );
    drop(reopened);
    drop(cva);
    fs::remove_dir_all(source.parent().unwrap()).unwrap();
}
