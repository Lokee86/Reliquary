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
        source_node_id: Some("root".into()),
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
    assert!(cva.memory(memory.id).is_some());
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
    assert!(reopened.memory(memory.id).is_some());
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
    let merged = Cva::open(output).unwrap();
    assert!(merged.conversation_tombstone("conversation").is_some());
    assert!(merged.memory(memory.id).is_some());
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
