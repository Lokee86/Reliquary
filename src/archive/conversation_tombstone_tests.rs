use crate::{
    ArchiveError, Branch, Cva, CvaReconcileConflict, CvaReconcileError, IncomingTurn, MemoryDraft,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reliquary-conversation-tombstone-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn turn(conversation_id: &str, id: &str, parent_id: Option<&str>) -> IncomingTurn {
    IncomingTurn {
        id: id.into(),
        conversation_id: conversation_id.into(),
        parent_id: parent_id.map(str::to_owned),
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 1,
        content: format!("content-{id}"),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}

fn create_base(path: &Path) {
    let mut cva = Cva::create_project(path).unwrap();
    cva.ingest_turn(turn("conversation", "root", None)).unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "conversation".into(),
        leaf_node_id: "root".into(),
        canonical: true,
    })
    .unwrap();
    cva.set_conversation_title("conversation", "Original".into())
        .unwrap();
    cva.sync().unwrap();
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: "Downstream memory".into(),
        content: "Must survive direct-only conversation deletion".into(),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: Some("conversation".into()),
        content_source_node_id: Some("root".into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(1),
        mutation_id: "downstream-memory".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn terminal_conversation_tombstone_reopens_with_historical_closure_only() {
    let path = test_dir().join("archive.rel");
    create_base(&path);
    let mut cva = Cva::open(&path).unwrap();
    let (memory, _) = cva.publish_memory(None, 0, memory_draft()).unwrap();
    let before = (cva.archive_version(), cva.latest_global_version());

    assert!(cva.tombstone_conversation("conversation").unwrap());
    let tombstone = cva.conversation_tombstone("conversation").unwrap();
    assert_eq!(tombstone.archive_version, before.0 + 1);
    assert_eq!(tombstone.global_version, before.1 + 1);
    assert_eq!(tombstone.closure.node_ids, vec!["root".to_owned()]);
    assert_eq!(tombstone.closure.branch_ids, vec!["main".to_owned()]);
    assert_eq!(tombstone.closure.metadata_archive_versions.len(), 1);
    assert!(!cva.tombstone_conversation("conversation").unwrap());
    assert_eq!(cva.archive_version(), before.0 + 1);
    assert!(cva.conversation_summaries().is_empty());
    assert!(cva.branches().is_empty());
    assert!(cva.conversation_metadata("conversation").is_none());
    assert!(matches!(
        cva.ingest_turn(turn("conversation", "later", Some("root"))),
        Err(ArchiveError::TombstonedConversation)
    ));
    assert!(cva.memory(memory.id).is_ok());
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert!(reopened.conversation_summaries().is_empty());
    assert!(reopened.branches().is_empty());
    assert_eq!(reopened.historical_branches().len(), 1);
    assert!(
        reopened
            .historical_conversation_metadata("conversation")
            .is_some()
    );
    assert_eq!(
        reopened
            .historical_conversation_turns("conversation", "root")
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        reopened.conversation_turns("conversation", "root"),
        Err(ArchiveError::TombstonedConversation)
    ));
    assert!(reopened.memory(memory.id).is_ok());
}

#[test]
fn reconcile_replays_noncompeting_delete_without_cascade() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    let output = dir.join("merged.rel");
    create_base(&left);
    fs::copy(&left, &right).unwrap();
    let memory = {
        let mut left_cva = Cva::open(&left).unwrap();
        left_cva
            .ingest_turn(turn("other", "other-root", None))
            .unwrap();
        let (memory, _) = left_cva.publish_memory(None, 0, memory_draft()).unwrap();
        left_cva.sync().unwrap();
        memory
    };
    {
        let mut right_cva = Cva::open(&right).unwrap();
        right_cva.tombstone_conversation("conversation").unwrap();
        right_cva.sync().unwrap();
    }

    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open(output).unwrap();
    assert!(merged.conversation_tombstone("conversation").is_some());
    assert!(merged.memory(memory.id).is_ok());
    assert_eq!(merged.conversation_summaries().len(), 1);
    assert_eq!(merged.conversation_summaries()[0].conversation_id, "other");
}

#[test]
fn reconcile_reports_update_delete_conflict_in_both_directions() {
    for delete_left in [false, true] {
        let dir = test_dir();
        let left = dir.join("left.rel");
        let right = dir.join("right.rel");
        let output = dir.join("merged.rel");
        create_base(&left);
        fs::copy(&left, &right).unwrap();
        let (delete_path, update_path) = if delete_left {
            (&left, &right)
        } else {
            (&right, &left)
        };
        let mut deleted = Cva::open(delete_path).unwrap();
        deleted.tombstone_conversation("conversation").unwrap();
        deleted.sync().unwrap();
        let mut updated = Cva::open(update_path).unwrap();
        updated
            .set_conversation_title("conversation", "Diverged".into())
            .unwrap();
        updated.sync().unwrap();

        let error = Cva::reconcile(&left, &right, &output).unwrap_err();
        assert!(matches!(
            error,
            CvaReconcileError::Conflict(CvaReconcileConflict::ConversationTombstone {
                ref conversation_id,
            }) if conversation_id == "conversation"
        ));
        assert!(!output.exists());
    }
}

#[test]
fn tombstone_crash_prefix_reclaim_and_reconcile_preserve_archive_history() {
    use crate::owner_lifecycle::{
        LifecycleError, LifecycleState, LifecycleTarget, TombstoneCoordinates,
    };

    let dir = test_dir();
    let source = dir.join("source.rel");
    let divergent = dir.join("divergent.rel");
    let reclaimed = dir.join("reclaimed.rel");
    let merged = dir.join("merged.rel");

    create_base(&source);
    let (memory, entity, attachment_id, original_closure, activity_positions) = {
        let mut cva = Cva::open(&source).unwrap();
        cva.set_conversation_title("conversation", "Revised".into())
            .unwrap();
        let ingested = cva
            .ingest_turn(IncomingTurn {
                id: "child".into(),
                conversation_id: "conversation".into(),
                parent_id: Some("root".into()),
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 2,
                content: "second turn".into(),
                attachments: vec![crate::IncomingAttachment {
                    filename: "notes.txt".into(),
                    mime_type: Some("text/plain".into()),
                    bytes: b"conversation attachment".to_vec(),
                }],
                project_attachments: Vec::new(),
            })
            .unwrap();
        let attachment_id = ingested.attachments[0].id;
        cva.append_branch(Branch {
            id: "main".into(),
            conversation_id: "conversation".into(),
            leaf_node_id: "child".into(),
            canonical: true,
        })
        .unwrap();
        cva.append_branch(Branch {
            id: "alternate".into(),
            conversation_id: "conversation".into(),
            leaf_node_id: "root".into(),
            canonical: false,
        })
        .unwrap();
        let (memory, _) = cva.publish_memory(None, 0, memory_draft()).unwrap();
        let entity = cva
            .publish_entity(
                None,
                0,
                crate::EntityDraft {
                    canonical_name: "Independent downstream entity".into(),
                    aliases: Vec::new(),
                    kind: "test".into(),
                    summary: "Survives direct-only Archive deletion".into(),
                    mutation_id: "conversation-tombstone-survivor".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap()
            .0;
        cva.link_file_to_memory(attachment_id, memory.id).unwrap();
        assert_eq!(
            cva.conversation_lifecycle_state("conversation").unwrap(),
            LifecycleState::Active
        );
        let activity_positions = ["root", "child"].map(|node| {
            cva.activity_position_for_turn("conversation", node)
                .unwrap()
        });
        let downstream_versions = (cva.memory_version(), cva.entity_version());
        cva.sync().unwrap();
        fs::copy(&source, &divergent).unwrap();
        let tombstone = {
            cva.tombstone_conversation("conversation").unwrap();
            cva.conversation_tombstone("conversation").unwrap().clone()
        };
        assert_eq!(
            cva.conversation_lifecycle_state("conversation").unwrap(),
            LifecycleState::Tombstoned(TombstoneCoordinates {
                global_version: Some(tombstone.global_version),
                domain_version: Some(tombstone.archive_version),
            })
        );
        let versions = (cva.archive_version(), cva.latest_global_version());
        assert!(!cva.tombstone_conversation("conversation").unwrap());
        assert_eq!(
            versions,
            (cva.archive_version(), cva.latest_global_version())
        );
        assert_eq!(
            downstream_versions,
            (cva.memory_version(), cva.entity_version())
        );
        assert_eq!(tombstone.closure.node_ids, vec!["child", "root"]);
        assert_eq!(tombstone.closure.branch_ids, vec!["alternate", "main"]);
        assert_eq!(tombstone.closure.metadata_archive_versions.len(), 2);
        assert_eq!(tombstone.closure.file_ids, vec![attachment_id]);
        (
            memory,
            entity,
            attachment_id,
            tombstone.closure,
            activity_positions,
        )
    };

    {
        let mut branch = Cva::open(&divergent).unwrap();
        branch
            .ingest_turn(turn("unrelated", "unrelated-root", None))
            .unwrap();
        branch.sync().unwrap();
    }

    let mut reopened = Cva::open(&source).unwrap();
    let tombstone = reopened
        .conversation_tombstone("conversation")
        .unwrap()
        .clone();
    assert_eq!(tombstone.closure, original_closure);
    assert!(reopened.branches().is_empty());
    assert!(reopened.conversation_summaries().is_empty());
    assert_eq!(reopened.historical_branches().len(), 2);
    assert_eq!(
        reopened
            .historical_conversation_turns("conversation", "child")
            .unwrap()
            .iter()
            .map(|turn| turn.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["root", "child"]
    );
    assert_eq!(
        ["root", "child"].map(|node| reopened
            .activity_position_for_turn("conversation", node)
            .unwrap()),
        activity_positions
    );
    assert_eq!(
        reopened
            .conversation_lifecycle_state("conversation")
            .unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(tombstone.global_version),
            domain_version: Some(tombstone.archive_version),
        })
    );
    assert_eq!(reopened.memory(memory.id).unwrap().id, memory.id);
    assert_eq!(reopened.entity(entity.id).unwrap().id, entity.id);
    assert_eq!(reopened.file_memory_links(attachment_id).len(), 1);
    assert_eq!(
        reopened.file_bytes(attachment_id).unwrap(),
        b"conversation attachment"
    );

    let pre_reclaim = (
        reopened.latest_global_version(),
        reopened.archive_version(),
        reopened.memory_version(),
        reopened
            .activity_cut_at_archive_version(reopened.archive_version())
            .unwrap(),
    );
    let report = reopened.reclaim_storage(&reclaimed).unwrap();
    assert!(report.output_file_bytes > 0);
    drop(reopened);

    let mut reclaimed_rel = Cva::open(&reclaimed).unwrap();
    assert_eq!(
        (
            reclaimed_rel.latest_global_version(),
            reclaimed_rel.archive_version(),
            reclaimed_rel.memory_version(),
            reclaimed_rel
                .activity_cut_at_archive_version(reclaimed_rel.archive_version())
                .unwrap(),
        ),
        pre_reclaim
    );
    assert_eq!(
        reclaimed_rel
            .conversation_tombstone("conversation")
            .unwrap()
            .closure,
        original_closure
    );
    assert_eq!(
        reclaimed_rel
            .historical_conversation_turns("conversation", "child")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(reclaimed_rel.memory(memory.id).unwrap().id, memory.id);
    assert_eq!(reclaimed_rel.entity(entity.id).unwrap().id, entity.id);
    assert_eq!(
        reclaimed_rel.file_bytes(attachment_id).unwrap(),
        b"conversation attachment"
    );
    assert!(matches!(
        reclaimed_rel.conversation_lifecycle_state("missing"),
        Err(LifecycleError::UnknownTarget(LifecycleTarget::Conversation(id))) if id == "missing"
    ));
    drop(reclaimed_rel);

    Cva::reconcile(&source, &divergent, &merged).unwrap();
    let mut reconciled = Cva::open(&merged).unwrap();
    assert_eq!(
        reconciled
            .conversation_tombstone("conversation")
            .unwrap()
            .closure,
        original_closure
    );
    assert_eq!(reconciled.historical_branches().len(), 2);
    assert_eq!(
        reconciled
            .historical_conversation_turns("conversation", "child")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(reconciled.memory(memory.id).unwrap().id, memory.id);
    assert_eq!(reconciled.entity(entity.id).unwrap().id, entity.id);
    assert_eq!(reconciled.file_memory_links(attachment_id).len(), 1);
    assert!(
        reconciled
            .conversation_summaries()
            .iter()
            .any(|summary| summary.conversation_id == "unrelated")
    );
}
