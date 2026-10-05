use super::*;

fn branch_runtime(path: std::path::PathBuf) -> InteractionRuntime {
    let mut runtime = seeded_runtime(path);
    runtime
        .cva
        .append_node(
            "alt-root".into(),
            "host-conversation".into(),
            None,
            "user".into(),
            2,
            "alternate root",
        )
        .unwrap();
    runtime
        .cva
        .append_branch(Branch {
            id: "alternate".into(),
            conversation_id: "host-conversation".into(),
            leaf_node_id: "alt-root".into(),
            canonical: false,
        })
        .unwrap();
    runtime.cva.sync().unwrap();
    runtime
}
fn branch_submit(
    gateway: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    id: &str,
    branch: &str,
) {
    let revision = gateway
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new("host-conversation").unwrap());
    gateway
        .submit_conversation(
            writer,
            owner,
            GatewayConversationSubmission {
                conversation_id: "host-conversation".into(),
                submission_id: id.into(),
                expected_conversation_revision: revision,
                expected_parent_node_id: Some(
                    if branch == "main" { "root" } else { "alt-root" }.into(),
                ),
                branch_id: branch.into(),
                content: format!("branch turn {id}"),
                attachments: vec![],
                client_timestamp_ns: None,
            },
        )
        .unwrap();
}

#[test]
fn blocked_handoff_serializes_same_branch_but_allows_other_branch_and_detach() {
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let gateway = Arc::new(gateway_with_host(
        test_path("branch-live-host-state"),
        host.clone(),
    ));
    let owner = gateway
        .mount_rel(branch_runtime(test_path("branch-live-host.rel")))
        .unwrap();
    let writer = gateway
        .attach_instance("branch-host-principal".into(), None, writer_grants(&owner))
        .unwrap();
    branch_submit(&gateway, &writer, &owner, "main-active", "main");
    branch_submit(&gateway, &writer, &owner, "main-next", "main");
    branch_submit(&gateway, &writer, &owner, "alt-active", "alternate");
    host.block_next_handoff();
    let process_gateway = gateway.clone();
    let process_owner = owner.clone();
    let process = std::thread::spawn(move || {
        process_gateway.process_accepted_conversation_submission(
            &process_owner,
            "host-conversation",
            "main-active",
        )
    });
    host.wait_for_handoff();
    assert!(
        gateway
            .process_accepted_conversation_submission(&owner, "host-conversation", "main-active")
            .is_err()
    );
    assert!(
        gateway
            .process_accepted_conversation_submission(&owner, "host-conversation", "main-next")
            .is_err()
    );
    let alt = gateway
        .process_accepted_conversation_submission(&owner, "host-conversation", "alt-active")
        .unwrap();
    assert_eq!(alt.status.state, SubmissionState::Pending);
    assert_eq!(
        host.handoff_count(),
        2,
        "other branch reaches the real adapter before main returns"
    );
    gateway
        .select_managed_conversation(&writer, &owner, "host-conversation", "alternate")
        .unwrap();
    gateway.detach_instance(&writer).unwrap();
    host.release_handoff();
    assert_eq!(
        process.join().unwrap().unwrap().status.state,
        SubmissionState::Pending
    );
    assert!(
        gateway
            .process_accepted_conversation_submission(&owner, "host-conversation", "main-next")
            .is_err(),
        "returning from dispatch must not release durable branch debt"
    );
    for id in ["main-active", "alt-active"] {
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new("host-conversation").unwrap(),
            submission_id: SubmissionId::new(id).unwrap(),
        };
        host.set_status(
            &conversation_host_operation_id(&owner, &key),
            HostOperationStatus::Completed,
        );
        let receipt = gateway
            .process_accepted_conversation_submission(&owner, "host-conversation", id)
            .unwrap();
        assert_eq!(receipt.status.state, SubmissionState::Committed);
        let capture = receipt.status.finalize_capture.unwrap();
        assert_eq!(
            capture.parent_node_id.as_deref(),
            Some(if id == "main-active" {
                "root"
            } else {
                "alt-root"
            })
        );
    }
    assert_eq!(host.handoff_count(), 2);
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn restart_retains_unknown_host_branch_debt_while_idle_branch_advances() {
    let state = test_path("branch-host-recovery-state");
    let path = test_path("branch-host-recovery.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let first = gateway_with_host(state.clone(), host);
    let owner = first.mount_rel(branch_runtime(path.clone())).unwrap();
    let writer = first
        .attach_instance("branch-host-principal".into(), None, writer_grants(&owner))
        .unwrap();
    branch_submit(&first, &writer, &owner, "host-debt", "main");
    branch_submit(&first, &writer, &owner, "after-debt", "main");
    branch_submit(&first, &writer, &owner, "idle-alt", "alternate");
    first
        .process_accepted_conversation_submission(&owner, "host-conversation", "host-debt")
        .unwrap();
    first.detach_instance(&writer).unwrap();
    drop(first.unmount_rel(&owner).unwrap());
    drop(first);
    let recovered = super::super::tests::gateway_at(state);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
            .unwrap(),
        owner
    );
    assert!(
        recovered
            .process_accepted_conversation_submission(&owner, "host-conversation", "after-debt")
            .is_err()
    );
    let debt = recovered
        .process_accepted_conversation_submission(&owner, "host-conversation", "host-debt")
        .unwrap();
    assert_eq!(
        debt.status.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Unknown
    );
    let alt = recovered
        .process_accepted_conversation_submission(&owner, "host-conversation", "idle-alt")
        .unwrap();
    assert_eq!(alt.status.state, SubmissionState::Committed);
    assert_eq!(
        alt.status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("alt-root")
    );
    assert!(
        recovered
            .process_accepted_conversation_submission(&owner, "host-conversation", "after-debt")
            .is_err()
    );
    drop(recovered.unmount_rel(&owner).unwrap());
}

#[test]
fn repeated_restart_preserves_unknown_cancel_intent_without_host_replay() {
    let state = test_path("p38-unknown-host-state");
    let path = test_path("p38-unknown-host.rel");
    let host = Arc::new(ControlledHost::new(&[
        RuntimeCapability::ConversationExecution,
    ]));
    let g = gateway_with_host(state.clone(), host.clone());
    let owner = g.mount_rel(branch_runtime(path.clone())).unwrap();
    let w = g
        .attach_instance(
            "host-recovery-principal".into(),
            None,
            writer_grants(&owner),
        )
        .unwrap();
    branch_submit(&g, &w, &owner, "uncertain", "main");
    branch_submit(&g, &w, &owner, "waiting", "main");
    branch_submit(&g, &w, &owner, "independent", "alternate");
    host.lose_next_ack();
    g.process_next_accepted_conversation_submission(&owner, "host-conversation")
        .unwrap()
        .unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("host-conversation").unwrap(),
        submission_id: SubmissionId::new("uncertain").unwrap(),
    };
    let operation_id = conversation_host_operation_id(&owner, &key);
    host.set_status(&operation_id, HostOperationStatus::Unknown);
    g.cancel_conversation_generation(&w, &owner, "host-conversation", &operation_id)
        .unwrap();
    let original = g
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .host_operation(&key, RuntimeCapability::ConversationExecution)
        .unwrap();
    assert_eq!(original.phase, DurableHostOperationPhase::Unknown);
    assert!(original.cancel_requested);
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g);
    for iteration in 0..2 {
        let g = super::super::tests::gateway_at(state.clone());
        g.mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
            .unwrap();
        let w = g
            .attach_instance("recovered-observer".into(), None, writer_grants(&owner))
            .unwrap();
        let before = g
            .conversation_observation_snapshot(&w, &owner, "host-conversation")
            .unwrap();
        assert_eq!(before.host_operations.len(), 1);
        assert_eq!(before.host_operations[0].operation_id, operation_id);
        assert_eq!(
            before.host_operations[0].phase,
            DurableHostOperationPhase::Unknown
        );
        assert!(before.host_operations[0].cancel_requested);
        if iteration == 0 {
            let independent = g
                .process_next_accepted_conversation_submission(&owner, "host-conversation")
                .unwrap()
                .unwrap();
            assert_eq!(independent.id.submission_id.as_str(), "independent");
            assert_eq!(independent.status.state, SubmissionState::Committed);
        }
        assert!(
            g.process_next_accepted_conversation_submission(&owner, "host-conversation")
                .unwrap()
                .is_none()
        );
        let resumed = g
            .resume_conversation_submission(&w, &owner, "host-conversation", "uncertain")
            .unwrap();
        let retained = &resumed.status.host_operations[&RuntimeCapability::ConversationExecution];
        assert_eq!(retained, &original);
        assert_eq!(resumed.status.state, SubmissionState::Pending);
        assert!(resumed.status.finalize_capture.is_none());
        assert_eq!(host.handoff_count(), 1);
        g.detach_instance(&w).unwrap();
        drop(g.unmount_rel(&owner).unwrap());
    }
    // Authoritative host completion can win after cancellation intent.
    host.set_status(&operation_id, HostOperationStatus::Completed);
    let g = gateway_with_host(state, host.clone());
    g.mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
        .unwrap();
    let w = g
        .attach_instance("host-reconciler".into(), None, writer_grants(&owner))
        .unwrap();
    let resolved = g
        .resume_conversation_submission(&w, &owner, "host-conversation", "uncertain")
        .unwrap();
    assert_eq!(resolved.status.state, SubmissionState::Committed);
    assert!(
        resolved.status.host_operations[&RuntimeCapability::ConversationExecution].cancel_requested
    );
    assert_eq!(host.handoff_count(), 1);
    let next = g
        .process_next_accepted_conversation_submission(&owner, "host-conversation")
        .unwrap()
        .unwrap();
    assert_eq!(next.id.submission_id.as_str(), "waiting");
    assert_eq!(
        host.handoffs()
            .iter()
            .filter(|(id, _)| id == &operation_id)
            .count(),
        1,
        "the uncertain operation is never handed off again"
    );
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}
