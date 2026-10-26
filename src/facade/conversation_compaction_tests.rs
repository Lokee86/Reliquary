use crate::{Cva, CvaRelation};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn test_path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reliquary-compaction-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join("project.rel")
}

#[test]
fn compaction_records_persist_inside_the_rel() {
    let path = test_path();
    let mut cva = Cva::create_project(&path).unwrap();
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "message-4".into(),
        "durable compacted context".into(),
        None,
    )
    .unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.conversation_compactions("conversation-1")[0].summary,
        "durable compacted context"
    );
}

#[test]
fn superseded_compactions_are_reclaimed_and_reused_without_file_growth() {
    let path = test_path();
    let mut cva = Cva::create_project(&path).unwrap();
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "message-1".into(),
        "a".repeat(1_500),
        None,
    )
    .unwrap();
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "message-2".into(),
        "b".repeat(4_000),
        Some("message-1"),
    )
    .unwrap();
    let grown_len = fs::metadata(&path).unwrap().len();

    cva.put_conversation_compaction(
        "conversation-1".into(),
        "message-3".into(),
        "c".repeat(100),
        Some("message-2"),
    )
    .unwrap();
    assert!(fs::metadata(&path).unwrap().len() < grown_len);
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "message-4".into(),
        "d".repeat(5_000),
        Some("message-3"),
    )
    .unwrap();

    assert!(fs::metadata(&path).unwrap().len() <= grown_len);
    assert_eq!(cva.conversation_compactions("conversation-1").len(), 1);
    assert_eq!(
        cva.conversation_compactions("conversation-1")[0].through_message_id,
        "message-4"
    );

    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    let records = reopened.conversation_compactions("conversation-1");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].through_message_id, "message-4");
}

#[test]
fn sibling_branch_compactions_can_coexist() {
    let path = test_path();
    let mut cva = Cva::create_project(&path).unwrap();
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "left-4".into(),
        "left summary".into(),
        None,
    )
    .unwrap();
    cva.put_conversation_compaction(
        "conversation-1".into(),
        "right-4".into(),
        "right summary".into(),
        None,
    )
    .unwrap();

    let records = cva.conversation_compactions("conversation-1");
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| record.through_message_id == "left-4")
    );
    assert!(
        records
            .iter()
            .any(|record| record.through_message_id == "right-4")
    );
}

#[test]
fn compaction_chunks_do_not_create_false_reconciliation_divergence() {
    let left = test_path();
    let right = left.with_file_name("project-copy.rel");
    let cva = Cva::create_project(&left).unwrap();
    drop(cva);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva
        .put_conversation_compaction(
            "conversation-1".into(),
            "message-4".into(),
            "derived summary".into(),
            None,
        )
        .unwrap();
    drop(left_cva);

    assert_eq!(
        Cva::compare(&left, &right).unwrap().relation,
        CvaRelation::Identical
    );
}
