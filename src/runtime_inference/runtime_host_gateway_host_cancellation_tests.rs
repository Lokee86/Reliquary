use super::*;

#[test]
fn running_host_cancel_is_cooperative_and_unknown_status_cannot_replay() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = gateway_with_host(test_path("p5b-running-cancel-state"), host.clone());
    let (owner, writer) = mount_writer(&gateway, test_path("p5b-running-cancel.rel"), "alice");
    submit(&gateway, &writer, &owner, "running-turn");
    let pending = gateway
        .process_next_conversation_submission(&writer, &owner, "host-conversation")
        .unwrap()
        .unwrap();
    assert_eq!(pending.status.state, SubmissionState::Pending);
    let (id, _) = host.handoffs().pop().unwrap();
    host.set_status(&id, HostOperationStatus::Running);
    let cancelled = gateway
        .cancel_conversation_generation(&writer, &owner, "host-conversation", &id)
        .unwrap();
    assert_eq!(cancelled.status.state, SubmissionState::Pending);
    assert_ne!(
        cancelled.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Cancelled
    );
    assert_eq!(host.cancel_count(), 1);
    assert_eq!(
        gateway
            .host_router
            .as_ref()
            .unwrap()
            .status(RuntimeCapability::ConversationExecution, &id)
            .unwrap(),
        HostOperationStatus::Running
    );
    assert!(!archive_has_submission(&gateway, &owner, "running-turn"));

    host.set_status(&id, HostOperationStatus::Unknown);
    let held = gateway
        .process_exact_conversation_submission_for_test(
            &writer,
            &owner,
            "host-conversation",
            "running-turn",
        )
        .unwrap();
    assert_eq!(held.status.state, SubmissionState::Pending);
    assert_eq!(
        gateway
            .capability_authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held
    );
    assert_eq!(host.handoff_count(), 1);
    // Completion of the already running stage can win; cancellation is not rollback.
    host.set_status(&id, HostOperationStatus::Completed);
    let completed = gateway
        .process_exact_conversation_submission_for_test(
            &writer,
            &owner,
            "host-conversation",
            "running-turn",
        )
        .unwrap();
    assert_eq!(completed.status.state, SubmissionState::Committed);
    assert!(archive_has_submission(&gateway, &owner, "running-turn"));
    gateway
        .process_exact_conversation_submission_for_test(
            &writer,
            &owner,
            "host-conversation",
            "running-turn",
        )
        .unwrap();
    assert_eq!(host.handoff_count(), 1);
}
