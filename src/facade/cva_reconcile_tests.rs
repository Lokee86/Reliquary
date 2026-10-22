use crate::{
    Cva, CvaReconcileError, CvaRelation, EchoEvent, EchoEventKind, InteractionRole,
    InteractionRuntime, InteractionStreamStatus,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-cva-reconcile-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_base(path: &Path) {
    Cva::create_project(path).unwrap().sync().unwrap();
}

#[test]
fn compare_identical_copies() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    create_base(&left);
    fs::copy(&left, &right).unwrap();

    let comparison = Cva::compare(&left, &right).unwrap();
    assert_eq!(comparison.relation, CvaRelation::Identical);
    assert_eq!(comparison.left_chunk_count, comparison.right_chunk_count);
    assert_eq!(comparison.common_chunk_count, comparison.left_chunk_count);
}

#[test]
fn compare_detects_one_side_ahead() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    create_base(&left);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva
        .append_node(
            "node-a".into(),
            "conversation-a".into(),
            None,
            "user".into(),
            1,
            "left-only",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let comparison = Cva::compare(&left, &right).unwrap();
    assert_eq!(comparison.relation, CvaRelation::LeftExtendsRight);
    assert_eq!(comparison.common_chunk_count, comparison.right_chunk_count);
}

#[test]
fn compare_detects_divergent_tails() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    create_base(&left);
    fs::copy(&left, &right).unwrap();

    for (path, id, content) in [
        (&left, "node-a", "left-only"),
        (&right, "node-b", "right-only"),
    ] {
        let mut cva = Cva::open(path).unwrap();
        cva.append_node(
            id.into(),
            "conversation-a".into(),
            None,
            "user".into(),
            1,
            content,
        )
        .unwrap();
        cva.sync().unwrap();
    }

    let comparison = Cva::compare(&left, &right).unwrap();
    assert_eq!(comparison.relation, CvaRelation::Diverged);
    assert!(comparison.common_chunk_count < comparison.left_chunk_count);
    assert!(comparison.common_chunk_count < comparison.right_chunk_count);
}

#[test]
fn divergent_reconcile_preserves_interrupted_interaction_streams() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    create_base(&left);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva
        .append_node(
            "left-node".into(),
            "left-conversation".into(),
            None,
            "user".into(),
            1,
            "left change",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let right_cva = Cva::open(&right).unwrap();
    let mut runtime = InteractionRuntime::new(right_cva);
    runtime.open_session("chat".into(), None).unwrap();
    runtime
        .begin_message("chat", "user-1".into(), InteractionRole::User, 2)
        .unwrap();
    runtime.append_text("chat", "user-1", "Question").unwrap();
    runtime.complete_message("chat", "user-1").unwrap();
    runtime
        .begin_message("chat", "assistant-1".into(), InteractionRole::Agent, 3)
        .unwrap();
    runtime
        .append_checkpointed_text("chat", "assistant-1", "Visible partial")
        .unwrap();
    runtime.interrupt_message("chat", "assistant-1").unwrap();
    drop(runtime);

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert!(result.canonical_change_required);
    let merged = Cva::open(output).unwrap();
    let streams = merged.interaction_stream_records();
    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].content, "Visible partial");
    assert_eq!(streams[0].status, InteractionStreamStatus::Interrupted);
}

#[test]
fn divergent_reconcile_preserves_echo_from_both_copies() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    let output = dir.join("merged.rel");
    let mut base = Cva::create_project(&left).unwrap();
    base.append_node(
        "assistant-common".into(),
        "conversation".into(),
        None,
        "assistant".into(),
        1,
        "common answer",
    )
    .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    for (path, sequence, echo, node) in [
        (&left, 0, "left reasoning", "left-node"),
        (&right, 1, "right commentary", "right-node"),
    ] {
        let mut rel = Cva::open(path).unwrap();
        rel.put_echo_event(EchoEvent {
            conversation_id: "conversation".into(),
            message_id: "assistant-common".into(),
            sequence,
            timestamp_ns: 2 + sequence as i64,
            model_round: Some(1),
            kind: if sequence == 0 {
                EchoEventKind::ReasoningTrace
            } else {
                EchoEventKind::Commentary
            },
            correlation_id: None,
            name: None,
            content: echo.into(),
        })
        .unwrap();
        rel.append_node(
            node.into(),
            format!("{node}-conversation"),
            None,
            "user".into(),
            3,
            node,
        )
        .unwrap();
        rel.sync().unwrap();
    }

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    let echo = merged.echo_events("conversation", "assistant-common");
    assert_eq!(echo.len(), 2);
    assert_eq!(echo[0].content, "left reasoning");
    assert_eq!(echo[1].content, "right commentary");
}

#[test]
fn divergent_reconcile_preserves_rel_metadata() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    let output = dir.join("merged.rel");
    let mut base = Cva::create(&left).unwrap();
    base.set_rel_metadata(
        Some("Program".into()),
        vec!["rel-parent-a".into(), "rel-parent-b".into()],
    )
    .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    for (path, id, content) in [
        (&left, "node-a", "left-only"),
        (&right, "node-b", "right-only"),
    ] {
        let mut rel = Cva::open(path).unwrap();
        rel.append_node(
            id.into(),
            "conversation-a".into(),
            None,
            "user".into(),
            1,
            content,
        )
        .unwrap();
        rel.sync().unwrap();
    }

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(&output).unwrap();
    assert_eq!(merged.rel_metadata().type_label.as_deref(), Some("Program"));
    assert_eq!(
        merged.rel_metadata().dependencies,
        vec!["rel-parent-a".to_string(), "rel-parent-b".to_string()]
    );
}

#[test]
fn compare_rejects_different_reliquary_owners_regardless_of_type_label() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    Cva::create_project(&left).unwrap();
    Cva::create_organization(&right).unwrap();

    assert!(matches!(
        Cva::compare(&left, &right),
        Err(CvaReconcileError::OwnerMismatch { .. })
    ));
}

#[test]
fn compare_rejects_different_owners() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    create_base(&left);
    create_base(&right);

    assert!(matches!(
        Cva::compare(&left, &right),
        Err(CvaReconcileError::OwnerMismatch { .. })
    ));
}
