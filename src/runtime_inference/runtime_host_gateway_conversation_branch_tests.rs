use super::*;

#[test]
fn mount_rejects_multiple_unresolved_finalizers_on_one_branch_before_publication() {
    let state = test_path("branch-conflict-state");
    let path = test_path("branch-conflict.rel");
    let first = gateway_at(state.clone());
    let owner = first.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let writer = writer(&first, &owner);
    submit_branch(&first, &writer, &owner, "conflict-one", "main", "root-one");
    submit_branch(&first, &writer, &owner, "conflict-two", "main", "root-one");
    freeze(&first, &owner, "conflict-one");
    freeze(&first, &owner, "conflict-two");
    drop(writer);
    // Simulate crash rather than graceful reconciliation of this invalid ledger.
    drop(first);
    let recovered = gateway_at(state);
    assert!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
            .is_err()
    );
    let archive = Cva::open(path).unwrap();
    assert!(
        !archive
            .archive()
            .has_node("conversation-one", "conflict-one")
    );
    assert!(
        !archive
            .archive()
            .has_node("conversation-one", "conflict-two")
    );
}

fn key(id: &str) -> ConversationSubmissionId {
    ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new(id).unwrap(),
    }
}
fn writer(gateway: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    gateway
        .attach_instance(
            "branch-principal".into(),
            None,
            grants(
                owner,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap()
}
fn submit_branch(
    gateway: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    id: &str,
    branch: &str,
    parent: &str,
) {
    let mut request = submission("conversation-one", id, parent);
    request.branch_id = branch.into();
    request.expected_conversation_revision = gateway
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new("conversation-one").unwrap());
    gateway.submit_conversation(writer, owner, request).unwrap();
}
fn freeze(gateway: &GatewayRuntime, owner: &str, id: &str) -> FinalizeCapture {
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "branch-principal".into(),
        policy_revision: 1,
    };
    let journal = gateway.conversation.journal(owner).unwrap();
    let mut journal = journal.lock().unwrap();
    let revision = journal.conversation_revision(&key(id).conversation_id);
    journal
        .claim_finalize(&key(id), revision, capture.clone())
        .unwrap();
    capture
}

#[test]
fn restart_rebuilds_finalizing_branch_and_other_branch_can_advance_without_viewers() {
    let state = test_path("branch-recovery-state");
    let path = test_path("branch-recovery.rel");
    let first = gateway_at(state.clone());
    let owner = first.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let writer = writer(&first, &owner);
    submit_branch(&first, &writer, &owner, "frozen-main", "main", "root-one");
    let capture = freeze(&first, &owner, "frozen-main");
    submit_branch(&first, &writer, &owner, "queued-main", "main", "root-one");
    submit_branch(
        &first,
        &writer,
        &owner,
        "queued-alt",
        "alternate",
        "root-one-alt",
    );
    first.detach_instance(&writer).unwrap();
    drop(first.unmount_rel(&owner).unwrap());
    drop(first);

    let recovered = gateway_at(state);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
            .unwrap(),
        owner
    );
    assert!(
        recovered
            .process_accepted_conversation_submission(&owner, "conversation-one", "queued-main")
            .is_err()
    );
    let alt = recovered
        .process_accepted_conversation_submission(&owner, "conversation-one", "queued-alt")
        .unwrap();
    assert_eq!(
        alt.status
            .finalize_capture
            .as_ref()
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("root-one-alt")
    );
    let main = recovered
        .process_accepted_conversation_submission(&owner, "conversation-one", "frozen-main")
        .unwrap();
    assert_eq!(main.status.finalize_capture, Some(capture));
    let queued = recovered
        .process_accepted_conversation_submission(&owner, "conversation-one", "queued-main")
        .unwrap();
    assert_eq!(
        queued
            .status
            .finalize_capture
            .as_ref()
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("frozen-main")
    );
    assert_eq!(
        recovered
            .process_accepted_conversation_submission(&owner, "conversation-one", "queued-alt")
            .unwrap(),
        alt
    );
    let archive = recovered.unmount_rel(&owner).unwrap();
    let branches = archive.archive().branches();
    assert!(
        branches
            .iter()
            .any(|b| b.id == "main" && b.leaf_node_id == "queued-main")
    );
    assert!(
        branches
            .iter()
            .any(|b| b.id == "alternate" && b.leaf_node_id == "queued-alt" && !b.canonical)
    );
}

#[test]
fn unfinished_postcommit_receipt_fences_only_its_branch() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    submit_branch(&gateway, &writer, &owner, "post-main", "main", "root-one");
    let capture = freeze(&gateway, &owner, "post-main");
    let journal = gateway.conversation.journal(&owner).unwrap();
    let payload = journal
        .lock()
        .unwrap()
        .submission(&key("post-main"))
        .unwrap()
        .clone();
    let token = gateway
        .authorize(&writer, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let receipt = gateway
        .with_rel(&token, |runtime| {
            archive_receipt_or_commit(runtime, &key("post-main"), &payload, &capture, &[])
        })
        .unwrap();
    journal
        .lock()
        .unwrap()
        .record_archive_commit(&key("post-main"), receipt)
        .unwrap();
    submit_branch(&gateway, &writer, &owner, "after-main", "main", "post-main");
    submit_branch(
        &gateway,
        &writer,
        &owner,
        "other-alt",
        "alternate",
        "root-one-alt",
    );
    assert!(
        gateway
            .process_accepted_conversation_submission(&owner, "conversation-one", "after-main")
            .is_err()
    );
    let alt = gateway
        .process_accepted_conversation_submission(&owner, "conversation-one", "other-alt")
        .unwrap();
    assert_eq!(alt.status.state, SubmissionState::Committed);
    let main = gateway
        .process_accepted_conversation_submission(&owner, "conversation-one", "post-main")
        .unwrap();
    assert!(main.status.post_commit_receipt.is_some());
    let after = gateway
        .process_accepted_conversation_submission(&owner, "conversation-one", "after-main")
        .unwrap();
    assert_eq!(
        after
            .status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("post-main")
    );
    gateway.detach_instance(&writer).unwrap();
    drop(journal);
    drop(gateway.unmount_rel(&owner).unwrap());
}
