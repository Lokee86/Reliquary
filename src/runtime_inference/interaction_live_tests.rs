use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Branch, Cva, InsomniaWorkerConfig, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};

fn setup(label: &str) -> (Arc<GatewayRuntime>, String, Arc<InstanceRuntime>) {
    let g = Arc::new(GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        Default::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        Default::default(),
    ));
    let mut cva = Cva::create_project(test_path(&format!("{label}.rel"))).unwrap();
    cva.append_node(
        "root".into(),
        "session".into(),
        None,
        "user".into(),
        1,
        "seed",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "session".into(),
        leaf_node_id: "root".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = attach(&g, &owner, "one");
    (g, owner, instance)
}
fn attach(g: &GatewayRuntime, owner: &str, principal: &str) -> Arc<InstanceRuntime> {
    g.attach_instance(
        principal.into(),
        None,
        BTreeMap::from([(
            owner.to_owned(),
            BTreeSet::from([
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelOwnPendingSubmission,
                OwnerGrant::CancelSharedGeneration,
                OwnerGrant::ManageRel,
            ]),
        )]),
    )
    .unwrap()
}
fn request(id: &str) -> GatewayConversationSubmission {
    GatewayConversationSubmission {
        conversation_id: "session".into(),
        submission_id: id.into(),
        expected_conversation_revision: 0,
        expected_parent_node_id: Some("root".into()),
        branch_id: "main".into(),
        content: "PRIVATE_USER_SENTINEL".into(),
        attachments: vec![],
        client_timestamp_ns: None,
    }
}
#[test]
fn two_adapters_share_durable_work_and_checkpoint_delivery() {
    let (g, owner, one) = setup("live-two-adapters");
    let two = attach(&g, &owner, "two");
    let a = g.live_runtime_adapter(one.clone());
    let b = g.live_runtime_adapter(two.clone());
    let mut first = a.open_session(&owner, "session", "main").unwrap();
    let mut second = b.open_session(&owner, "session", "main").unwrap();
    first.submit_turn(request("turn")).unwrap();
    let accepted = g
        .process_next_conversation_submission(&one, &owner, "session")
        .unwrap()
        .unwrap();
    assert_eq!(accepted.id.submission_id.as_str(), "turn");
    let generation = g
        .start_conversation_generation(&owner, "session", "turn")
        .unwrap()
        .status
        .generation
        .unwrap();
    first.watch_generation(&generation.operation_id, 4).unwrap();
    second
        .watch_generation(&generation.operation_id, 4)
        .unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 1, "雪")
        .unwrap();
    assert_eq!(first.next_event().unwrap(), second.next_event().unwrap());
    first.close().unwrap();
    assert!(matches!(first.next_event(), Err(LiveRuntimeError::Closed)));
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 2, "雪🙂")
        .unwrap();
    let resumed = second.resume_stream(4).unwrap();
    assert_eq!(resumed.checkpoint.unwrap().text, "雪🙂");
    assert!(second.next_event().unwrap().is_none());
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 3, "雪🙂!")
        .unwrap();
    match second.next_event().unwrap().unwrap() {
        LiveStreamEvent::Checkpoint { checkpoint, cursor } => {
            assert_eq!(checkpoint.text, "!");
            assert_eq!(
                checkpoint.shape,
                LiveCheckpointShape::Delta {
                    after_checkpoint_sequence: 2
                }
            );
            assert!(cursor.delivery_sequence > resumed.cursor.delivery_sequence);
        }
        other => panic!("unexpected {other:?}"),
    }
}
#[test]
fn adapter_replacement_does_not_replay_or_duplicate_admission() {
    let (g, owner, instance) = setup("live-replacement");
    let a = g.live_runtime_adapter(instance.clone());
    let mut first = a.open_session(&owner, "session", "main").unwrap();
    let original = first.submit_turn(request("same")).unwrap();
    first.close().unwrap();
    let b = g.live_runtime_adapter(instance.clone());
    let second = b.open_session(&owner, "session", "main").unwrap();
    let retry = second.submit_turn(request("same")).unwrap();
    assert_eq!(original, retry);
    assert!(
        g.process_next_conversation_submission(&instance, &owner, "session")
            .unwrap()
            .is_some()
    );
    assert!(
        g.process_next_conversation_submission(&instance, &owner, "session")
            .unwrap()
            .is_none()
    );
}
#[test]
fn pending_cancellation_is_principal_scoped_and_close_does_not_cancel() {
    let (g, owner, one) = setup("live-cancel-pending");
    let two = attach(&g, &owner, "other");
    let a = g.live_runtime_adapter(one.clone());
    let mut first = a.open_session(&owner, "session", "main").unwrap();
    first.submit_turn(request("pending")).unwrap();
    let b = g.live_runtime_adapter(two);
    let second = b.open_session(&owner, "session", "main").unwrap();
    assert!(second.cancel_pending("pending").is_err());
    first.close().unwrap();
    let replacement = a.open_session(&owner, "session", "main").unwrap();
    let receipt = replacement.cancel_pending("pending").unwrap();
    assert_eq!(
        receipt.status.state,
        crate::multiplexing_journal::SubmissionState::Cancelled
    );
}
#[test]
fn overflow_requires_snapshot_and_stale_epoch_cannot_resume() {
    let (g, owner, instance) = setup("live-overflow");
    let a = g.live_runtime_adapter(instance.clone());
    let mut session = a.open_session(&owner, "session", "main").unwrap();
    session.submit_turn(request("overflow")).unwrap();
    g.process_next_conversation_submission(&instance, &owner, "session")
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, "session", "overflow")
        .unwrap()
        .status
        .generation
        .unwrap();
    session
        .watch_generation(&generation.operation_id, 1)
        .unwrap();
    for (sequence, text) in [(1, "a"), (2, "ab")] {
        g.checkpoint_conversation_generation(
            &owner,
            "session",
            &generation.operation_id,
            sequence,
            text,
        )
        .unwrap();
    }
    assert!(matches!(
        session.next_event().unwrap(),
        Some(LiveStreamEvent::ResyncRequired { .. })
    ));
    assert_eq!(
        session.resume_stream(1).unwrap().checkpoint.unwrap().text,
        "ab"
    );
    g.quiesce_rel(&owner).unwrap();
    g.reopen_rel(&owner).unwrap();
    assert_eq!(
        session.resume_stream(1).unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
}
#[test]
fn diagnostics_and_errors_omit_content_and_provider_details() {
    let (g, owner, instance) = setup("live-diagnostics");
    let adapter = g.live_runtime_adapter(instance);
    let session = adapter.open_session(&owner, "session", "main").unwrap();
    session.submit_turn(request("private")).unwrap();
    let diagnostic = format!("{:?}", session.diagnostics().unwrap());
    assert!(!diagnostic.contains("PRIVATE_USER_SENTINEL"));
    let err = unavailable(ReliquaryRuntimeHostError::Operation(
        "SECRET_PROVIDER_PATH".into(),
    ));
    assert!(!format!("{err:?}: {err}").contains("SECRET_PROVIDER_PATH"));
}

#[test]
fn native_running_cancellation_and_revocation_use_existing_owner_authority() {
    let (g, owner, instance) = setup("live-native-cancel");
    let adapter = g.live_runtime_adapter(instance.clone());
    let session = adapter.open_session(&owner, "session", "main").unwrap();
    session.submit_turn(request("native")).unwrap();
    g.process_next_conversation_submission(&instance, &owner, "session")
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, "session", "native")
        .unwrap()
        .status
        .generation
        .unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 1, "prefix")
        .unwrap();
    let receipt = session.cancel_generation(&generation.operation_id).unwrap();
    assert_eq!(
        receipt.status.generation.unwrap().phase,
        crate::multiplexing_journal::DurableGenerationPhase::Interrupted
    );
    g.replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    assert_eq!(
        session.turn_status("native").unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
    assert_eq!(
        session.diagnostics().unwrap_err(),
        LiveRuntimeError::FreshSessionRequired
    );
}

#[test]
fn completion_status_survives_checkpoint_normalization() {
    let (g, owner, instance) = setup("live-completion");
    let adapter = g.live_runtime_adapter(instance.clone());
    let mut session = adapter.open_session(&owner, "session", "main").unwrap();
    session.submit_turn(request("complete")).unwrap();
    g.process_next_conversation_submission(&instance, &owner, "session")
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, "session", "complete")
        .unwrap()
        .status
        .generation
        .unwrap();
    session
        .watch_generation(&generation.operation_id, 8)
        .unwrap();
    g.checkpoint_conversation_generation(&owner, "session", &generation.operation_id, 1, "answer")
        .unwrap();
    session.next_event().unwrap().unwrap();
    g.complete_conversation_generation(&owner, "session", &generation.operation_id)
        .unwrap();
    let status = match session.next_event().unwrap().unwrap() {
        LiveStreamEvent::Checkpoint { checkpoint, .. } => checkpoint.generation.status,
        LiveStreamEvent::Status { generation, .. } => generation.status,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(status, LiveGenerationStatus::Completed);
}

#[path = "interaction_live_acceptance_tests.rs"]
mod acceptance;
