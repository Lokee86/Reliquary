use super::*;
use crate::ConversationPolicy::VisibleAutoBranch;

const CONVERSATION: &str = "conversation-one";
const ROOT: &str = "root-one";

fn auto_writer(
    gateway: &GatewayRuntime,
    owner: &str,
    principal: &str,
    manage: bool,
) -> Arc<InstanceRuntime> {
    let mut capabilities = vec![OwnerGrant::ViewRel, OwnerGrant::SubmitConversation];
    if manage {
        capabilities.push(OwnerGrant::ManageRel);
    }
    gateway
        .attach_instance(principal.to_owned(), None, grants(owner, &capabilities))
        .unwrap()
}

fn enable_auto_branch(gateway: &GatewayRuntime, manager: &InstanceRuntime, owner: &str) {
    assert!(
        gateway
            .set_conversation_policy(manager, owner, CONVERSATION, VisibleAutoBranch)
            .unwrap()
    );
    assert_eq!(
        gateway
            .conversation_policy(manager, owner, CONVERSATION)
            .unwrap(),
        (VisibleAutoBranch, 2)
    );
}

fn submit_stale_candidate(
    gateway: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    id: &str,
) -> GatewaySubmissionReceipt {
    let conversation = ConversationId::new(CONVERSATION).unwrap();
    let revision = gateway
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&conversation);
    let mut request = submission(CONVERSATION, id, ROOT);
    request.expected_conversation_revision = revision;
    gateway.submit_conversation(writer, owner, request).unwrap()
}

fn freeze_conflict(
    gateway: &GatewayRuntime,
    owner: &str,
    key: &ConversationSubmissionId,
) -> (Submission, FinalizeCapture) {
    let journal = gateway.conversation.journal(owner).unwrap();
    let submission = journal.lock().unwrap().submission(key).unwrap().clone();
    let authority = OperationAuthority::capture(gateway, owner).unwrap();
    let capture = with_operation(gateway, &authority, |runtime| {
        let (policy, policy_revision) = runtime.cva().conversation_policy(CONVERSATION);
        let target = super::super::finalization::finalization_target(
            runtime,
            CONVERSATION,
            &submission,
            policy,
        )?;
        Ok(FinalizeCapture {
            branch_id: target.branch_id,
            branch_label: target.branch_label,
            parent_node_id: target.parent_node_id,
            committed_timestamp_ns: now_ns(),
            verified_principal_id: submission.principal_id.clone(),
            policy_revision,
        })
    })
    .unwrap();
    let revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&key.conversation_id);
    journal
        .lock()
        .unwrap()
        .claim_finalize(key, revision, capture.clone())
        .unwrap();
    (submission, capture)
}

#[test]
fn two_writers_fork_stale_head_and_only_the_submitter_selection_moves() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let first = auto_writer(&gateway, &owner, "auto-first-principal", true);
    let second = auto_writer(&gateway, &owner, "auto-second-principal", false);
    let viewer = gateway
        .attach_instance(
            "auto-viewer".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    enable_auto_branch(&gateway, &first, &owner);
    gateway
        .select_managed_conversation(&viewer, &owner, CONVERSATION, "main")
        .unwrap();

    let first_receipt = submit_stale_candidate(&gateway, &first, &owner, "auto-first");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let second_receipt = submit_stale_candidate(&gateway, &second, &owner, "auto-second");

    let first_commit = gateway
        .process_next_conversation_submission(&first, &owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let second_commit = gateway
        .process_next_conversation_submission(&first, &owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert_eq!(first_commit.id, first_receipt.id);
    assert_eq!(second_commit.id, second_receipt.id);

    let first_capture = first_commit.status.finalize_capture.as_ref().unwrap();
    assert_eq!(first_capture.branch_id, "main");
    assert!(first_capture.branch_label.is_none());
    assert_eq!(first_capture.parent_node_id.as_deref(), Some(ROOT));
    assert_eq!(first_capture.policy_revision, 2);

    let second_capture = second_commit.status.finalize_capture.as_ref().unwrap();
    assert_ne!(second_capture.branch_id, "main");
    assert!(second_capture.branch_id.starts_with("auto-"));
    assert!(second_capture.branch_label.is_some());
    assert_eq!(second_capture.parent_node_id.as_deref(), Some(ROOT));
    assert_eq!(second_capture.policy_revision, 2);
    assert_eq!(
        second_commit
            .status
            .archive_receipt
            .as_ref()
            .unwrap()
            .branch_id,
        second_capture.branch_id
    );

    assert_eq!(
        gateway
            .instance_managed_state(&first, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some("main")
    );
    assert_eq!(
        gateway
            .instance_managed_state(&second, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some(second_capture.branch_id.as_str())
    );
    assert_eq!(
        gateway
            .instance_managed_state(&viewer, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some("main")
    );

    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &authority, |runtime| {
        let branches = runtime.cva().archive().branches();
        let main = branches
            .iter()
            .find(|branch| branch.conversation_id == CONVERSATION && branch.id == "main")
            .unwrap();
        assert_eq!(main.leaf_node_id, "auto-first");
        assert!(main.canonical);
        let fork = branches
            .iter()
            .find(|branch| {
                branch.conversation_id == CONVERSATION && branch.id == second_capture.branch_id
            })
            .unwrap();
        assert_eq!(fork.leaf_node_id, "auto-second");
        assert!(!fork.canonical);
        assert_eq!(
            runtime
                .cva()
                .archive()
                .nodes
                .get(CONVERSATION, "auto-second")
                .unwrap()
                .parent_id
                .as_deref(),
            Some(ROOT)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn three_writers_leave_one_main_winner_and_two_distinct_visible_conflict_branches() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let first = auto_writer(&gateway, &owner, "auto-order-first", true);
    let second = auto_writer(&gateway, &owner, "auto-order-second", false);
    let third = auto_writer(&gateway, &owner, "auto-order-third", false);
    enable_auto_branch(&gateway, &first, &owner);

    let first_receipt = submit_stale_candidate(&gateway, &first, &owner, "auto-order-a");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let second_receipt = submit_stale_candidate(&gateway, &second, &owner, "auto-order-b");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let third_receipt = submit_stale_candidate(&gateway, &third, &owner, "auto-order-c");

    let commits = (0..3)
        .map(|_| {
            gateway
                .process_next_conversation_submission(&first, &owner, CONVERSATION)
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(commits[0].id, first_receipt.id);
    assert_eq!(commits[1].id, second_receipt.id);
    assert_eq!(commits[2].id, third_receipt.id);

    let captures = commits
        .iter()
        .map(|receipt| receipt.status.finalize_capture.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(captures[0].branch_id, "main");
    assert!(captures[0].branch_label.is_none());
    for capture in &captures[1..] {
        assert_ne!(capture.branch_id, "main");
        assert!(capture.branch_label.is_some());
        assert_eq!(capture.parent_node_id.as_deref(), Some(ROOT));
        assert_eq!(capture.policy_revision, 2);
    }
    assert_ne!(captures[1].branch_id, captures[2].branch_id);

    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &authority, |runtime| {
        let branches = runtime.cva().archive().branches();
        assert_eq!(
            branches
                .iter()
                .filter(|branch| {
                    branch.conversation_id == CONVERSATION && branch.id.starts_with("auto-")
                })
                .count(),
            2
        );
        assert_eq!(
            branches
                .iter()
                .find(|branch| branch.conversation_id == CONVERSATION && branch.id == "main")
                .unwrap()
                .leaf_node_id,
            "auto-order-a"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn restart_before_or_after_auto_branch_publication_reuses_the_frozen_branch_identity() {
    for publish_before_crash in [false, true] {
        let state = test_path(if publish_before_crash {
            "auto-branch-after-publication-state"
        } else {
            "auto-branch-before-publication-state"
        });
        let archive = test_path(if publish_before_crash {
            "auto-branch-after-publication.rel"
        } else {
            "auto-branch-before-publication.rel"
        });
        let first_gateway = gateway_at(state.clone());
        let owner = first_gateway
            .mount_rel(seeded_runtime_at(archive.clone()))
            .unwrap();
        let first = auto_writer(&first_gateway, &owner, "auto-crash-first", true);
        let stale = auto_writer(&first_gateway, &owner, "auto-crash-stale", false);
        enable_auto_branch(&first_gateway, &first, &owner);

        submit_stale_candidate(&first_gateway, &first, &owner, "auto-crash-winner");
        std::thread::sleep(std::time::Duration::from_millis(1));
        let stale_receipt =
            submit_stale_candidate(&first_gateway, &stale, &owner, "auto-crash-stale");
        first_gateway
            .process_next_conversation_submission(&first, &owner, CONVERSATION)
            .unwrap()
            .unwrap();

        let journal = first_gateway.conversation.journal(&owner).unwrap();
        let (submission, capture) = freeze_conflict(&first_gateway, &owner, &stale_receipt.id);
        assert_ne!(capture.branch_id, "main");
        assert!(capture.branch_label.is_some());
        assert_eq!(capture.parent_node_id.as_deref(), Some(ROOT));

        if publish_before_crash {
            let authority = OperationAuthority::capture(&first_gateway, &owner).unwrap();
            with_operation(&first_gateway, &authority, |runtime| {
                archive_receipt_or_commit(runtime, &stale_receipt.id, &submission, &capture, &[])?;
                Ok(())
            })
            .unwrap();
            assert_eq!(
                journal
                    .lock()
                    .unwrap()
                    .status(&stale_receipt.id)
                    .unwrap()
                    .state,
                SubmissionState::Finalizing
            );
        }

        let execution = first_gateway
            .rel_executions
            .write()
            .unwrap()
            .remove(&owner)
            .unwrap();
        drop(execution);
        drop(journal);
        drop(stale);
        drop(first);
        drop(first_gateway);

        let recovered = gateway_at(state);
        assert_eq!(
            recovered
                .mount_rel(InteractionRuntime::new(Cva::open(&archive).unwrap()))
                .unwrap(),
            owner
        );
        let recovery_writer = auto_writer(&recovered, &owner, "auto-crash-recovery", false);
        let recovered_receipt = recovered
            .resume_conversation_submission(
                &recovery_writer,
                &owner,
                CONVERSATION,
                stale_receipt.id.submission_id.as_str(),
            )
            .unwrap();
        assert_eq!(recovered_receipt.status.state, SubmissionState::Committed);
        assert_eq!(
            recovered_receipt.status.finalize_capture.as_ref(),
            Some(&capture)
        );
        assert_eq!(
            recovered_receipt
                .status
                .archive_receipt
                .as_ref()
                .unwrap()
                .branch_id,
            capture.branch_id
        );

        let authority = OperationAuthority::capture(&recovered, &owner).unwrap();
        with_operation(&recovered, &authority, |runtime| {
            let branches = runtime.cva().archive().branches();
            let matching = branches
                .iter()
                .filter(|branch| {
                    branch.conversation_id == CONVERSATION && branch.id == capture.branch_id
                })
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1);
            assert_eq!(matching[0].leaf_node_id, "auto-crash-stale");
            assert!(!matching[0].canonical);
            assert_eq!(
                branches
                    .iter()
                    .find(|branch| branch.conversation_id == CONVERSATION && branch.id == "main")
                    .unwrap()
                    .leaf_node_id,
                "auto-crash-winner"
            );
            Ok(())
        })
        .unwrap();

        let replay = recovered
            .resume_conversation_submission(
                &recovery_writer,
                &owner,
                CONVERSATION,
                stale_receipt.id.submission_id.as_str(),
            )
            .unwrap();
        assert_eq!(replay, recovered_receipt);
    }
}

#[test]
fn detached_submitter_does_not_block_visible_conflict_finalization() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let first = auto_writer(&gateway, &owner, "auto-detach-first", true);
    let stale = auto_writer(&gateway, &owner, "auto-detach-stale", false);
    enable_auto_branch(&gateway, &first, &owner);

    submit_stale_candidate(&gateway, &first, &owner, "auto-detach-winner");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let stale_receipt = submit_stale_candidate(&gateway, &stale, &owner, "auto-detach-stale");
    gateway
        .process_next_conversation_submission(&first, &owner, CONVERSATION)
        .unwrap()
        .unwrap();
    gateway.detach_instance(&stale).unwrap();

    let committed = gateway
        .process_next_conversation_submission(&first, &owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert_eq!(committed.id, stale_receipt.id);
    let capture = committed.status.finalize_capture.as_ref().unwrap();
    assert_ne!(capture.branch_id, "main");
    assert!(capture.branch_label.is_some());
    assert_eq!(capture.parent_node_id.as_deref(), Some(ROOT));
    assert_eq!(committed.status.state, SubmissionState::Committed);
}

#[test]
fn visible_auto_branch_rejects_an_unrelated_retained_parent_without_publication() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let manager = auto_writer(&gateway, &owner, "auto-invalid-manager", true);
    enable_auto_branch(&gateway, &manager, &owner);

    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new(CONVERSATION).unwrap(),
        submission_id: SubmissionId::new("auto-invalid-parent").unwrap(),
    };
    let content = "retained invalid auto-branch parent".to_owned();
    let entry = Submission {
        id: key.clone(),
        principal_id: manager.principal_id().to_owned(),
        scope_fingerprint: fingerprint(&json!({
            "rel_id": owner, "conversation_id": CONVERSATION, "branch_id": "main",
        }))
        .unwrap(),
        payload_hash: Submission::canonical_payload_hash(&content, &[]).unwrap(),
        request_metadata: json!({
            "branch_id": "main",
            "expected_parent_node_id": "root-one-alt"
        }),
        content,
        attachments: vec![],
        client_timestamp_ns: None,
    };
    let journal = gateway.conversation.journal(&owner).unwrap();
    journal.lock().unwrap().enqueue(entry, now_ns(), 0).unwrap();

    assert!(
        gateway
            .process_next_conversation_submission(&manager, &owner, CONVERSATION)
            .is_err()
    );
    let status = journal.lock().unwrap().status(&key).unwrap();
    assert_eq!(status.state, SubmissionState::Pending);
    assert!(status.finalize_capture.is_none());

    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &authority, |runtime| {
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node(CONVERSATION, "auto-invalid-parent")
        );
        assert_eq!(
            runtime
                .cva()
                .archive()
                .branches()
                .iter()
                .filter(|branch| {
                    branch.conversation_id == CONVERSATION && branch.id.starts_with("auto-")
                })
                .count(),
            0
        );
        Ok(())
    })
    .unwrap();
}
