use super::*;

fn writer(gateway: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    gateway
        .attach_instance(
            "scheduler-principal".into(),
            None,
            grants(
                owner,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap()
}

fn submit(
    gateway: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    conversation: &str,
    id: &str,
    branch: &str,
    parent: &str,
    client_time: i64,
) {
    let mut request = submission(conversation, id, parent);
    request.branch_id = branch.into();
    request.client_timestamp_ns = Some(client_time);
    request.expected_conversation_revision = gateway
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new(conversation).unwrap());
    gateway.submit_conversation(writer, owner, request).unwrap();
}

#[test]
fn scheduler_uses_durable_order_ignores_delayed_client_time_and_deduplicates_retry() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "first",
        "main",
        "root-one",
        100,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-two",
        "other",
        "main",
        "root-two",
        0,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "delayed",
        "alternate",
        "root-one-alt",
        -100,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "delayed",
        "alternate",
        "root-one-alt",
        -100,
    );
    let journal = gateway.conversation.journal(&owner).unwrap();
    let conversation = ConversationId::new("conversation-one").unwrap();
    assert!(
        gateway
            .resume_conversation_submission(&writer, &owner, "conversation-one", "delayed")
            .is_err()
    );
    let order = journal.lock().unwrap().pending_order(&conversation);
    assert_eq!(order.len(), 2);
    assert_eq!(order[0].submission_id.as_str(), "first");
    for expected in order {
        let receipt = gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .unwrap();
        assert_eq!(receipt.id, expected);
        assert_eq!(receipt.status.state, SubmissionState::Committed);
    }
    assert!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        journal
            .lock()
            .unwrap()
            .pending_order(&ConversationId::new("conversation-two").unwrap())
            .len(),
        1
    );
    gateway.detach_instance(&writer).unwrap();
    // Work does not need its submitting attachment.
    assert_eq!(
        gateway
            .process_next_accepted_conversation_submission(&owner, "conversation-two")
            .unwrap()
            .unwrap()
            .id
            .submission_id
            .as_str(),
        "other"
    );
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn scheduler_skips_live_branch_and_reserves_other_branch_atomically() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "busy",
        "main",
        "root-one",
        0,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "later-main",
        "main",
        "root-one",
        0,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "eligible",
        "alternate",
        "root-one-alt",
        0,
    );
    let journal = gateway.conversation.journal(&owner).unwrap();
    let coordinator = gateway
        .conversation
        .coordinator(&owner, "conversation-one")
        .unwrap();
    let conversation = ConversationId::new("conversation-one").unwrap();
    let (first, lease) = coordinator
        .acquire_next(&journal, &conversation)
        .unwrap()
        .unwrap();
    assert_eq!(first.submission_id.as_str(), "busy");
    let receipt = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(receipt.id.submission_id.as_str(), "eligible");
    assert!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .is_none()
    );
    let remaining = journal.lock().unwrap().pending_order(&conversation);
    assert_eq!(
        remaining
            .iter()
            .map(|k| k.submission_id.as_str())
            .collect::<Vec<_>>(),
        ["busy", "later-main"]
    );
    drop(lease);
    assert_eq!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .unwrap()
            .id,
        first
    );
    assert_eq!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .unwrap()
            .id
            .submission_id
            .as_str(),
        "later-main"
    );
    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

// Controlled journal admissions exercise equal verified timestamps without sleeps
// or client-clock authority. Selection still uses the production coordinator.
#[test]
fn scheduler_preserves_equal_timestamp_ties_and_exact_order_after_restart() {
    let state = test_path("scheduler-ties-state");
    let rel_id = RelOwnerId::from_uuid(Uuid::new_v4());
    let conversation = ConversationId::new("ties").unwrap();
    let expected;
    {
        let mut journal = RelJournal::open(&state, rel_id).unwrap();
        for id in ["z", "a", "m", "b", "q", "c"] {
            let content = format!("turn {id}");
            let entry = Submission {
                id: ConversationSubmissionId {
                    conversation_id: conversation.clone(),
                    submission_id: SubmissionId::new(id).unwrap(),
                },
                principal_id: "principal".into(),
                scope_fingerprint: "scope".into(),
                payload_hash: Submission::canonical_payload_hash(&content, &[]).unwrap(),
                request_metadata: json!({"branch_id": "main"}),
                content,
                attachments: vec![],
                client_timestamp_ns: Some(-999),
            };
            let revision = journal.conversation_revision(&conversation);
            journal.enqueue(entry.clone(), 77, revision).unwrap();
            assert!(matches!(
                journal.enqueue(entry, 1, revision),
                Ok(EnqueueOutcome::Idempotent(_))
            ));
        }
        expected = journal.pending_order(&conversation);
        assert_eq!(journal.statuses().len(), expected.len());
        assert!(
            journal
                .statuses()
                .iter()
                .all(|(_, s)| s.verified_receive_ns == 77)
        );
        journal.compact().unwrap();
    }
    let journal = Arc::new(Mutex::new(RelJournal::open(&state, rel_id).unwrap()));
    let coordinator = Arc::new(ConversationCoordinator::new());
    assert_eq!(
        journal.lock().unwrap().pending_order(&conversation),
        expected
    );
    for key in expected {
        let (selected, lease) = coordinator
            .acquire_next(&journal, &conversation)
            .unwrap()
            .unwrap();
        assert_eq!(selected, key);
        // A second dispatcher cannot claim this or a later entry on its branch.
        assert!(
            coordinator
                .acquire_next(&journal, &conversation)
                .unwrap()
                .is_none()
        );
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&conversation);
        journal.cancel(&key, revision).unwrap();
        drop(journal);
        drop(lease);
    }
    assert!(
        coordinator
            .acquire_next(&journal, &conversation)
            .unwrap()
            .is_none()
    );
}

#[test]
fn scheduler_resumes_captured_finalization_without_reordering_ancestry() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "captured",
        "main",
        "root-one",
        0,
    );
    let journal = gateway.conversation.journal(&owner).unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("captured").unwrap(),
    };
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: 123,
        verified_principal_id: "scheduler-principal".into(),
        policy_revision: 1,
    };
    {
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&key.conversation_id);
        journal
            .claim_finalize(&key, revision, capture.clone())
            .unwrap();
    }
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "after",
        "main",
        "root-one",
        -999,
    );
    let receipt = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(receipt.id, key);
    assert_eq!(receipt.status.finalize_capture, Some(capture));
    assert_eq!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .unwrap()
            .status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("captured")
    );
    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn scheduler_skips_unknown_host_debt_and_reopens_same_branch_only_after_resolution() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "held-host",
        "main",
        "root-one",
        0,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "after-host",
        "main",
        "root-one",
        0,
    );
    submit(
        &gateway,
        &writer,
        &owner,
        "conversation-one",
        "idle-alt",
        "alternate",
        "root-one-alt",
        0,
    );
    let journal = gateway.conversation.journal(&owner).unwrap();
    let held = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("held-host").unwrap(),
    };
    {
        let mut journal = journal.lock().unwrap();
        journal
            .begin_host_handoff(
                &held,
                RuntimeCapability::ConversationExecution,
                "held-operation".into(),
                [1; 32],
            )
            .unwrap();
        journal
            .record_host_status(
                &held,
                RuntimeCapability::ConversationExecution,
                HostOperationStatus::Unknown,
            )
            .unwrap();
    }
    let before = journal.lock().unwrap().status(&held).unwrap();
    assert_eq!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .unwrap()
            .id
            .submission_id
            .as_str(),
        "idle-alt"
    );
    assert!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .unwrap()
            .is_none()
    );
    assert_eq!(journal.lock().unwrap().status(&held).unwrap(), before);
    // Resume cannot select the waiting fresh submission.
    assert!(
        gateway
            .resume_conversation_submission(&writer, &owner, "conversation-one", "after-host")
            .is_err()
    );
    {
        let mut journal = journal.lock().unwrap();
        journal
            .record_host_status(
                &held,
                RuntimeCapability::ConversationExecution,
                HostOperationStatus::Completed,
            )
            .unwrap();
    }
    let coordinator = gateway
        .conversation
        .coordinator(&owner, "conversation-one")
        .unwrap();
    let (selected, lease) = coordinator
        .acquire_next(&journal, &held.conversation_id)
        .unwrap()
        .unwrap();
    assert_eq!(selected, held);
    assert!(
        coordinator
            .acquire_next(&journal, &held.conversation_id)
            .unwrap()
            .is_none()
    );
    drop(lease);
    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}
