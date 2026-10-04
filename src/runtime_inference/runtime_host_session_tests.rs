use crate::{
    Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRole, InteractionRuntime,
    ReliquaryRuntimeHost, ReliquaryRuntimeRoutes,
};
use std::path::PathBuf;
use uuid::Uuid;

fn test_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-managed-session-{label}-{}.rel",
        Uuid::new_v4()
    ))
}

fn host(label: &str) -> ReliquaryRuntimeHost {
    let cva = Cva::create_project(test_path(label)).unwrap();
    ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    )
    .unwrap()
}

#[test]
fn managed_session_owns_active_switch_and_reopen_state() {
    let host = host("switch");
    let first = host.start_conversation_session().unwrap();
    let receipt = host
        .persist_active_message(
            &first.session_id,
            "first question".into(),
            InteractionRole::User,
            None,
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        host.active_session_id().unwrap().as_deref(),
        Some(first.session_id.as_str())
    );

    let second = host.start_conversation_session().unwrap();
    assert_ne!(second.session_id, first.session_id);
    assert_eq!(
        host.active_session_id().unwrap().as_deref(),
        Some(second.session_id.as_str())
    );
    let first_summary = host
        .conversation_summaries()
        .unwrap()
        .into_iter()
        .find(|summary| summary.conversation_id == first.session_id)
        .unwrap();
    assert!(!first_summary.active);

    host.reopen_conversation_session(first.session_id.clone(), receipt.turn.node.id.clone())
        .unwrap();
    assert!(host.reopened_session_pending(&first.session_id).unwrap());
    host.mark_reopened_session_resumed(&first.session_id)
        .unwrap();
    assert!(!host.reopened_session_pending(&first.session_id).unwrap());
}

#[test]
fn managed_message_and_stream_lifecycle_is_reliquary_owned() {
    let host = host("stream");
    let session = host.start_conversation_session().unwrap();
    let user = host
        .persist_active_message(
            &session.session_id,
            "question".into(),
            InteractionRole::User,
            None,
            Vec::new(),
        )
        .unwrap();
    let assistant = host
        .begin_active_assistant_stream(&session.session_id)
        .unwrap();
    host.checkpoint_active_assistant_stream(&session.session_id, &assistant, "answer")
        .unwrap();
    let completion = host
        .complete_active_assistant_stream(&session.session_id, &assistant)
        .unwrap();

    let transcript = host
        .conversation_transcript(&session.session_id, &completion.receipt.turn.node.id)
        .unwrap();
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].message_id, user.turn.node.id);
    assert_eq!(transcript[1].content, "answer");
}

#[test]
fn managed_message_rejects_non_active_session() {
    let host = host("guard");
    let first = host.start_conversation_session().unwrap();
    let second = host.start_conversation_session().unwrap();

    let error = host
        .persist_active_message(
            &first.session_id,
            "stale".into(),
            InteractionRole::User,
            None,
            Vec::new(),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("conversation is not the active session"));

    host.persist_active_message(
        &second.session_id,
        "current".into(),
        InteractionRole::User,
        None,
        Vec::new(),
    )
    .unwrap();
}

#[test]
fn mount_repairs_stale_durable_active_conversation_flags() {
    let path = test_path("stale-active");
    let mut cva = Cva::create_project(&path).unwrap();
    cva.append_node(
        "u1".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "stale",
    )
    .unwrap();
    cva.set_conversation_active("conversation", true).unwrap();
    cva.sync().unwrap();
    let owner = cva.owner_id().unwrap();

    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(cva)).unwrap();
    host.set_active_rel(&owner).unwrap();

    let summary = host
        .conversation_summaries()
        .unwrap()
        .into_iter()
        .find(|summary| summary.conversation_id == "conversation")
        .unwrap();
    assert!(!summary.active);
}

#[test]
fn unmount_closes_managed_session_and_clears_durable_active_flag() {
    let path = test_path("unmount");
    let cva = Cva::create_project(&path).unwrap();
    let owner = cva.owner_id().unwrap();
    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(cva)).unwrap();
    host.set_active_rel(&owner).unwrap();

    let session = host.start_conversation_session().unwrap();
    host.persist_active_message(
        &session.session_id,
        "question".into(),
        InteractionRole::User,
        None,
        Vec::new(),
    )
    .unwrap();

    let cva = host.unmount_rel(&owner).unwrap();
    let summary = cva
        .conversation_summaries()
        .into_iter()
        .find(|summary| summary.conversation_id == session.session_id)
        .unwrap();
    assert!(!summary.active);
    assert_eq!(cva.episodes_for_conversation(&session.session_id).len(), 1);
}
