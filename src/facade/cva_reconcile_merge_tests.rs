use crate::{
    Branch, Cva, CvaReconcileConflict, CvaReconcileError, IncomingAttachment, IncomingTurn,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-cva-merge-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_workspace(path: &Path) {
    Cva::create_project(path).unwrap().sync().unwrap();
}

fn append(cva: &mut Cva, id: &str, conversation: &str, parent: Option<&str>, content: &str) {
    cva.append_node(
        id.into(),
        conversation.into(),
        parent.map(str::to_owned),
        "user".into(),
        1,
        content,
    )
    .unwrap();
}

fn append_synced(path: &Path, id: &str, conversation: &str, content: &str) {
    let mut cva = Cva::open(path).unwrap();
    append(&mut cva, id, conversation, None, content);
    cva.sync().unwrap();
}

#[test]
fn reconcile_merges_unrelated_divergent_nodes() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    create_workspace(&left);
    fs::copy(&left, &right).unwrap();
    append_synced(&left, "left", "left-c", "left");
    append_synced(&right, "right", "right-c", "right");

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_archive_records, 1);
    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.stats().nodes, 2);
}

#[test]
fn reconcile_replays_ingested_turn_attachments() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    create_workspace(&left);
    fs::copy(&left, &right).unwrap();
    append_synced(&left, "left", "left-c", "left");

    let mut right_cva = Cva::open(&right).unwrap();
    right_cva
        .ingest_turn(IncomingTurn {
            id: "right".into(),
            conversation_id: "right-c".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 2,
            content: "photo".into(),
            attachments: vec![IncomingAttachment {
                filename: "site.jpg".into(),
                mime_type: Some("image/jpeg".into()),
                bytes: b"field-photo".to_vec(),
            }],
            project_attachments: Vec::new(),
        })
        .unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open(output).unwrap();
    let files = merged.files_for_source("right-c", "right");
    assert_eq!(files.len(), 1);
    assert_eq!(merged.file_bytes(files[0].id).unwrap(), b"field-photo");
}

#[test]
fn reconcile_replays_conversation_metadata() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    create_workspace(&left);
    append_synced(&left, "root", "c", "root");
    fs::copy(&left, &right).unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    right_cva
        .set_conversation_title("c", "Imported title".into())
        .unwrap();
    right_cva
        .set_conversation_policy("c", crate::ConversationPolicy::VisibleAutoBranch)
        .unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    assert_eq!(
        merged.conversation_summaries()[0].title.as_deref(),
        Some("Imported title")
    );
    assert_eq!(
        merged.conversation_policy("c"),
        (crate::ConversationPolicy::VisibleAutoBranch, 2)
    );
}

#[test]
fn reconcile_surfaces_conflicting_branch_revisions() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    create_workspace(&left);
    let mut base = Cva::open(&left).unwrap();
    append(&mut base, "root", "c", None, "root");
    base.append_branch(Branch {
        id: "main".into(),
        conversation_id: "c".into(),
        leaf_node_id: "root".into(),
        canonical: true,
    })
    .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    for (path, id) in [(&left, "left"), (&right, "right")] {
        let mut cva = Cva::open(path).unwrap();
        append(&mut cva, id, "c", Some("root"), id);
        cva.append_branch(Branch {
            id: "main".into(),
            conversation_id: "c".into(),
            leaf_node_id: id.into(),
            canonical: true,
        })
        .unwrap();
        cva.sync().unwrap();
    }

    assert!(matches!(
        Cva::reconcile(&left, &right, &output),
        Err(CvaReconcileError::Conflict(CvaReconcileConflict::Branch {
            ref conversation_id,
            ref branch_id,
            ref existing_leaf_node_id,
            ref incoming_leaf_node_id,
        })) if conversation_id == "c"
            && branch_id == "main"
            && existing_leaf_node_id == "left"
            && incoming_leaf_node_id == "right"
    ));
    assert!(!output.exists());
}
