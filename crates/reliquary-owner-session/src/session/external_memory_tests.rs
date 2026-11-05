use super::*;
use crate::records::ExternalReferenceSnapshotManifest;
use crate::records::external_index::ExternalMessageRef;
use reliquary_container::ReliquaryScope;

#[test]
fn intent_and_partial_memory_resume_with_provenance_and_no_archive() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!(
        "external-resume-{}.rel",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let owner = [104; 16];
    let mut writer = crate::RelWriter::create_path(&path, ReliquaryScope::Project, owner).unwrap();
    writer
        .session
        .archive
        .accept_turn(
            &mut writer.container,
            reliquary_archive::TurnInput {
                id: "source-turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 5,
                content: "prior accepted activity".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let reference = ExternalMessageRef {
        source_namespace: "host".into(),
        conversation_id: "conversation".into(),
        message_id: "message".into(),
        content_hash: [9; 32],
    };
    writer
        .record_external_reference_snapshot(
            "alice",
            ExternalReferenceSnapshotManifest {
                namespace: "host".into(),
                contract_version: 1,
                watermark: 1,
                descriptors: vec![ExternalMessageDescriptor {
                    reference: reference.clone(),
                    role: "user".into(),
                    principal_id: Some("alice".into()),
                    timestamp_ns: 10,
                    active: true,
                }],
            },
        )
        .unwrap();
    let draft = MemoryDraft {
        title: "Preferred tool".into(),
        content: "The user prefers Atlas.".into(),
        category: "preference".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "extracted".into(),
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
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: "external-insomnia:test".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    };
    let entries = vec![ExternalMemoryDraft {
        draft: draft.clone(),
        authority_source: ExternalChunkRef {
            message: reference,
            start: 0,
            end: 10,
        },
        content_sources: vec![],
        grounding_sources: vec![],
    }];
    let window = [7; 32];
    let intent = new_intent(owner, "alice", window, &entries).unwrap();
    writer
        .session
        .external_memories
        .install_intent(&mut writer.container, intent, 1)
        .unwrap();
    let published = writer
        .session
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft, 20)
        .unwrap();
    writer.container.sync().unwrap();
    let id = published.revision.record.id;
    drop(writer);
    let admission = crate::GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = crate::RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
    assert!(
        writer
            .read_session()
            .external_memory_provenance(id)
            .is_some()
    );
    let receipt = writer
        .resume_external_memory_publication("alice", window)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.memory_ids(), &[id]);
    let freshness_id = reliquary_freshness_store::FreshnessMemoryId(id.0);
    assert!(writer.session.freshness.has_durable_birth(freshness_id));
    assert!(
        writer
            .session
            .freshness
            .record_at(freshness_id, 1)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        writer
            .publish_external_memory("alice", window, entries.clone())
            .unwrap(),
        receipt
    );
    assert_eq!(writer.read_session().rel_turn_count(), 1);
    assert!(
        writer
            .record_external_accepted_use(
                "external-pre-dream-use",
                vec![freshness_id],
                1,
                FreshnessEventProof {
                    graph_version: 0,
                    community_generation: None,
                    community_graph_version: None
                }
            )
            .unwrap()
    );
    assert!(
        !writer
            .record_external_accepted_use(
                "external-pre-dream-use",
                vec![freshness_id],
                1,
                FreshnessEventProof {
                    graph_version: 0,
                    community_generation: None,
                    community_graph_version: None
                }
            )
            .unwrap()
    );
    let mut conflict = entries.clone();
    conflict[0].draft.content.push_str(" conflicting wording");
    assert!(
        writer
            .publish_external_memory("alice", window, conflict)
            .is_err()
    );
    let empty = writer
        .publish_external_memory("alice", [8; 32], vec![])
        .unwrap();
    assert!(empty.memory_ids().is_empty());
    assert_eq!(
        writer
            .resume_external_memory_publication("alice", [8; 32])
            .unwrap(),
        Some(empty)
    );
    writer
        .record_external_reference_snapshot(
            "alice",
            ExternalReferenceSnapshotManifest {
                namespace: "host".into(),
                contract_version: 1,
                watermark: 2,
                descriptors: vec![],
            },
        )
        .unwrap();
    let receipt_bytes = writer
        .container
        .read_frames()
        .unwrap()
        .into_iter()
        .find(|frame| frame.payload.starts_with(b"RELXMRP3"))
        .unwrap()
        .payload;
    let wrong_path = root.join(format!("external-forged-{}.rel", std::process::id()));
    let mut wrong =
        crate::RelWriter::create_path(&wrong_path, ReliquaryScope::Project, [105; 16]).unwrap();
    wrong.container.append(&receipt_bytes).unwrap();
    wrong.container.sync().unwrap();
    drop(wrong);
    assert!(
        crate::RelSession::open_path(
            &wrong_path,
            &crate::GraphAdmission::for_replay([105; 16], [], [], [], [], []),
            [],
            [],
        )
        .is_err()
    );
    std::fs::remove_file(wrong_path).unwrap();
    drop(writer);
    let rel = crate::RelSession::open_path(&path, &admission, [], []).unwrap();
    assert_eq!(rel.external_memory_receipt("alice", window), Some(&receipt));
    assert!(rel.freshness.has_durable_birth(freshness_id));
    assert_eq!(
        rel.accepted_external_use("external-pre-dream-use")
            .unwrap()
            .sources,
        vec![freshness_id]
    );
    assert_eq!(rel.memory_history(id).unwrap().unwrap().revisions.len(), 1);
    drop(rel);
    std::fs::remove_file(path).unwrap();
}
