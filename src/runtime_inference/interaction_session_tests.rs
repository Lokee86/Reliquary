use crate::{
    Cva, DEFAULT_EPISODE_INACTIVITY_NS, EpisodeConfig, EpisodePolicy, InteractionAttachment,
    InteractionError, InteractionRole, InteractionRuntime,
};
use std::fs;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-interaction-session-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

#[test]
fn streamed_message_assembles_before_one_durable_publication() {
    let path = test_path();
    let cva = Cva::create(&path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("s1".into(), None).unwrap();
    runtime
        .begin_message("s1", "u1".into(), InteractionRole::User, 10)
        .unwrap();
    runtime.append_text("s1", "u1", "Review ").unwrap();
    runtime.append_text("s1", "u1", "this plan.").unwrap();
    runtime
        .attach(
            "s1",
            "u1",
            InteractionAttachment {
                filename: "plan.pdf".into(),
                mime_type: Some("application/pdf".into()),
                bytes: b"pdf".to_vec(),
            },
        )
        .unwrap();

    assert_eq!(runtime.cva().archive_version(), 0);
    assert_eq!(runtime.cva().stats().nodes, 0);
    let receipt = runtime.complete_message("s1", "u1").unwrap();
    assert_eq!(receipt.archive_version, 1);
    assert_eq!(receipt.turn.node.parent_id, None);
    assert_eq!(
        runtime.session("s1").unwrap().leaf_message_id.as_deref(),
        Some("u1")
    );
    drop(runtime);

    let mut reopened = Cva::open(path).unwrap();
    let files = reopened.files_for_source("s1", "u1");
    assert_eq!(files.len(), 1);
    assert_eq!(reopened.file_bytes(files[0].id).unwrap(), b"pdf");
}

#[test]
fn session_chains_messages_and_resumes_from_durable_leaf_after_reopen() {
    let path = test_path();
    let cva = Cva::create(&path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("s1".into(), None).unwrap();
    runtime
        .begin_message("s1", "u1".into(), InteractionRole::User, 10)
        .unwrap();
    runtime.append_text("s1", "u1", "Question").unwrap();
    runtime.complete_message("s1", "u1").unwrap();
    drop(runtime);

    let cva = Cva::open(&path).unwrap();
    let mut resumed = InteractionRuntime::new(cva);
    assert!(matches!(
        resumed.open_session("s1".into(), None),
        Err(InteractionError::ResumeRequired)
    ));
    let session = resumed
        .open_session("s1".into(), Some("u1".into()))
        .unwrap();
    assert_eq!(session.leaf_message_id.as_deref(), Some("u1"));
    resumed
        .begin_message("s1", "a1".into(), InteractionRole::Agent, 20)
        .unwrap();
    resumed
        .append_checkpointed_text("s1", "a1", "Answer")
        .unwrap();
    assert_eq!(resumed.cva().rel_turn_count(), 1);
    assert_eq!(resumed.cva().activity_position_for_turn("s1", "a1"), None);
    let receipt = resumed.complete_message("s1", "a1").unwrap();
    assert_eq!(receipt.turn.node.parent_id.as_deref(), Some("u1"));
    assert_eq!(receipt.turn.node.role, "assistant");
    assert!(receipt.inserted);
    assert_eq!(receipt.activity_position, Some(2));
    assert_eq!(receipt.rel_turn_count, 2);

    let bad = resumed.open_session("s2".into(), Some("missing".into()));
    assert!(matches!(bad, Err(InteractionError::MissingResumeMessage)));
}

#[test]
fn incomplete_or_cancelled_stream_never_becomes_source_history() {
    let path = test_path();
    let cva = Cva::create(&path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("s1".into(), None).unwrap();
    runtime
        .begin_message("s1", "u1".into(), InteractionRole::User, 10)
        .unwrap();
    runtime.append_text("s1", "u1", "partial").unwrap();
    assert!(matches!(
        runtime.close_session("s1"),
        Err(InteractionError::MessageInProgress)
    ));
    runtime.cancel_message("s1", "u1").unwrap();
    runtime.close_session("s1").unwrap();
    assert_eq!(runtime.cva().archive_version(), 0);
    assert_eq!(runtime.cva().stats().nodes, 0);
    drop(runtime);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.stats().nodes, 0);
}

#[test]
fn live_episode_scheduling_is_explicitly_after_turn_acknowledgement() {
    let path = test_path();
    let cva = Cva::create(&path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("s1".into(), None).unwrap();

    runtime
        .begin_message("s1", "u1".into(), InteractionRole::User, 10)
        .unwrap();
    runtime.append_text("s1", "u1", "Remember this").unwrap();
    let user = runtime
        .complete_live_message("s1", "u1", EpisodePolicy::default(), 10)
        .unwrap();
    assert!(user.scheduling.is_ok());
    runtime
        .begin_message("s1", "a1".into(), InteractionRole::Agent, 20)
        .unwrap();
    runtime.append_text("s1", "a1", "Acknowledged").unwrap();
    let agent = runtime
        .complete_live_message("s1", "a1", EpisodePolicy::default(), 20)
        .unwrap();
    assert!(agent.scheduling.is_ok());
    assert_eq!(agent.receipt.archive_version, 2);
    assert_eq!(runtime.cva().stats().episodes, 0);

    let due = runtime
        .finalize_inactive_session(
            "s1",
            EpisodePolicy::default(),
            20 + DEFAULT_EPISODE_INACTIVITY_NS,
        )
        .unwrap()
        .unwrap();
    assert_eq!(due.episodes.created.len(), 1);
    assert_eq!(due.queued.len(), 1);
    assert_eq!(runtime.cva().stats().episodes, 1);
    assert_eq!(runtime.cva().insomnia_stats().pending, 1);
}

#[test]
fn live_completion_preserves_durable_receipt_when_scheduling_fails() {
    let path = test_path();
    let cva = Cva::create(&path).unwrap();
    let mut runtime = InteractionRuntime::new(cva);
    runtime.open_session("s1".into(), None).unwrap();
    runtime
        .begin_message("s1", "u1".into(), InteractionRole::User, 10)
        .unwrap();
    runtime.append_text("s1", "u1", "Durable first").unwrap();

    let completion = runtime
        .complete_live_message(
            "s1",
            "u1",
            EpisodePolicy {
                episode: EpisodeConfig { max_input_bytes: 0 },
                inactivity_ns: DEFAULT_EPISODE_INACTIVITY_NS,
            },
            10,
        )
        .unwrap();
    assert_eq!(completion.receipt.archive_version, 1);
    assert!(completion.receipt.inserted);
    assert_eq!(completion.receipt.activity_position, Some(1));
    assert_eq!(completion.receipt.rel_turn_count, 1);
    assert!(matches!(
        completion.scheduling,
        Err(InteractionError::Insomnia(_))
    ));
    drop(runtime);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.stats().nodes, 1);
    assert_eq!(reopened.archive_version(), 1);
}
