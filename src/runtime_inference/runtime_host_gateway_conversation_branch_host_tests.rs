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
