use super::*;
use crate::ConversationPolicy::{SequentialChronological, VisibleAutoBranch};

fn key(conversation: &str, submission: &str) -> ConversationSubmissionId {
    ConversationSubmissionId {
        conversation_id: ConversationId::new(conversation).unwrap(),
        submission_id: SubmissionId::new(submission).unwrap(),
    }
}

#[test]
fn policy_mutation_requires_current_manage_rel_on_the_exact_owner() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let other = gateway.mount_rel(seeded_runtime()).unwrap();
    for capabilities in [
        vec![OwnerGrant::ViewRel],
        vec![OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
        vec![OwnerGrant::ViewRel, OwnerGrant::ManageProject],
        vec![OwnerGrant::ViewRel, OwnerGrant::CancelSharedGeneration],
    ] {
        let instance = gateway
            .attach_instance("non-manager".into(), None, grants(&owner, &capabilities))
            .unwrap();
        assert!(
            gateway
                .set_conversation_policy(&instance, &owner, "conversation-one", VisibleAutoBranch)
                .is_err()
        );
        assert_eq!(
            gateway
                .conversation_policy(&instance, &owner, "conversation-one")
                .unwrap(),
            (SequentialChronological, 1)
        );
        gateway.detach_instance(&instance).unwrap();
    }
    let manager = gateway
        .attach_instance(
            "manager".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
        )
        .unwrap();
    assert!(
        gateway
            .set_conversation_policy(&manager, &other, "conversation-one", VisibleAutoBranch)
            .is_err()
    );
    assert!(
        gateway
            .set_conversation_policy(&manager, &owner, "missing", VisibleAutoBranch)
            .is_err()
    );
    assert!(
        gateway
            .set_conversation_policy(&manager, &owner, "conversation-one", VisibleAutoBranch)
            .unwrap()
    );
    assert!(
        !gateway
            .set_conversation_policy(&manager, &owner, "conversation-one", VisibleAutoBranch)
            .unwrap()
    );
    assert_eq!(
        gateway
            .conversation_policy(&manager, &owner, "conversation-one")
            .unwrap(),
        (VisibleAutoBranch, 2)
    );
    gateway
        .replace_instance_grants(&manager, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    assert!(
        gateway
            .set_conversation_policy(
                &manager,
                &owner,
                "conversation-one",
                SequentialChronological
            )
            .is_err()
    );
    gateway.detach_instance(&manager).unwrap();
    assert!(
        gateway
            .set_conversation_policy(
                &manager,
                &owner,
                "conversation-one",
                SequentialChronological
            )
            .is_err()
    );
    assert!(
        gateway
            .conversation_policy(&manager, &owner, "conversation-one")
            .is_err()
    );
    drop(gateway.unmount_rel(&other).unwrap());
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn policy_changes_only_pending_capture_and_recovery_preserves_frozen_ancestry() {
    let state_dir = test_path("policy-gateway-state");
    let archive_path = test_path("policy-owner.rel");
    let first = gateway_at(state_dir.clone());
    let owner = first
        .mount_rel(seeded_runtime_at(archive_path.clone()))
        .unwrap();
    let manager = first
        .attach_instance(
            "policy-principal".into(),
            None,
            grants(
                &owner,
                &[
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::ManageRel,
                ],
            ),
        )
        .unwrap();
    let conversation = "conversation-one";
    assert_eq!(
        first
            .conversation_policy(&manager, &owner, conversation)
            .unwrap(),
        (SequentialChronological, 1)
    );
    first
        .submit_conversation(
            &manager,
            &owner,
            submission(conversation, "committed-policy", "root-one"),
        )
        .unwrap();
    let committed = first
        .process_exact_conversation_submission_for_test(
            &manager,
            &owner,
            conversation,
            "committed-policy",
        )
        .unwrap();
    let committed_capture = committed.status.finalize_capture.clone().unwrap();
    assert_eq!(committed_capture.policy_revision, 1);
    assert!(
        first
            .set_conversation_policy(&manager, &owner, conversation, VisibleAutoBranch)
            .unwrap()
    );
    let journal = first.conversation.journal(&owner).unwrap();
    for id in ["frozen-policy", "pending-policy"] {
        let mut request = submission(conversation, id, "committed-policy");
        request.expected_conversation_revision = journal
            .lock()
            .unwrap()
            .conversation_revision(&ConversationId::new(conversation).unwrap());
        first
            .submit_conversation(&manager, &owner, request)
            .unwrap();
    }
    let frozen_key = key(conversation, "frozen-policy");
    let pending_key = key(conversation, "pending-policy");
    let frozen_capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("committed-policy".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "policy-principal".into(),
        policy_revision: 2,
    };
    {
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&frozen_key.conversation_id);
        journal
            .claim_finalize(&frozen_key, revision, frozen_capture.clone())
            .unwrap();
        assert_eq!(
            journal.submission(&pending_key).unwrap().request_metadata["admission"]["policy_revision"],
            1
        );
    }
    // One committed, one Finalizing and one Pending entry coexist in this conversation.
    assert!(
        first
            .set_conversation_policy(&manager, &owner, conversation, SequentialChronological)
            .unwrap()
    );
    {
        let journal = journal.lock().unwrap();
        assert_eq!(
            journal
                .status(&key(conversation, "committed-policy"))
                .unwrap()
                .finalize_capture,
            Some(committed_capture.clone())
        );
        assert_eq!(
            journal.status(&frozen_key).unwrap().finalize_capture,
            Some(frozen_capture.clone())
        );
        assert_eq!(
            journal.status(&pending_key).unwrap().state,
            SubmissionState::Pending
        );
        assert!(
            journal
                .status(&pending_key)
                .unwrap()
                .finalize_capture
                .is_none()
        );
    }
    let mut retry = submission(conversation, "pending-policy", "committed-policy");
    retry.expected_conversation_revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&pending_key.conversation_id);
    let retried = first
        .submit_conversation(&manager, &owner, retry.clone())
        .unwrap();
    assert_eq!(retried.status.state, SubmissionState::Pending);
    journal.lock().unwrap().compact().unwrap();
    assert_eq!(
        first.submit_conversation(&manager, &owner, retry).unwrap(),
        retried
    );
    drop(journal);
    first.detach_instance(&manager).unwrap();
    drop(first.unmount_rel(&owner).unwrap());
    drop(first);

    let recovered = gateway_at(state_dir);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(&archive_path).unwrap()))
            .unwrap(),
        owner
    );
    let observer = recovered
        .attach_instance(
            "policy-observer".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    assert_eq!(
        recovered
            .conversation_policy(&observer, &owner, conversation)
            .unwrap(),
        (SequentialChronological, 3)
    );
    let journal = recovered.conversation.journal(&owner).unwrap();
    {
        let journal = journal.lock().unwrap();
        let frozen = journal.status(&frozen_key).unwrap();
        assert_eq!(frozen.state, SubmissionState::Finalizing);
        assert_eq!(frozen.finalize_capture, Some(frozen_capture));
        assert_eq!(
            journal
                .status(&key(conversation, "committed-policy"))
                .unwrap()
                .finalize_capture,
            Some(committed_capture)
        );
        assert_eq!(
            journal.status(&pending_key).unwrap().state,
            SubmissionState::Pending
        );
    }
    let frozen_receipt = recovered
        .process_accepted_conversation_submission(&owner, conversation, "frozen-policy")
        .unwrap();
    assert_eq!(frozen_receipt.status.state, SubmissionState::Committed);
    assert_eq!(
        frozen_receipt
            .status
            .finalize_capture
            .as_ref()
            .unwrap()
            .policy_revision,
        2
    );
    assert_eq!(
        frozen_receipt
            .status
            .finalize_capture
            .as_ref()
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("committed-policy")
    );
    let receipt = recovered
        .process_accepted_conversation_submission(&owner, conversation, "pending-policy")
        .unwrap();
    assert_eq!(receipt.status.state, SubmissionState::Committed);
    let capture = receipt.status.finalize_capture.unwrap();
    assert_eq!(
        capture.policy_revision, 3,
        "Pending work captures current policy, not admission policy"
    );
    assert_eq!(capture.parent_node_id.as_deref(), Some("frozen-policy"));
    drop(journal);
    recovered.detach_instance(&observer).unwrap();
    drop(recovered.unmount_rel(&owner).unwrap());
}

#[test]
fn nondefault_policy_revision_is_captured_by_production_finalization() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let manager = gateway
        .attach_instance(
            "policy-writer".into(),
            None,
            grants(
                &owner,
                &[
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::ManageRel,
                ],
            ),
        )
        .unwrap();
    gateway
        .set_conversation_policy(&manager, &owner, "conversation-one", VisibleAutoBranch)
        .unwrap();
    gateway
        .submit_conversation(
            &manager,
            &owner,
            submission("conversation-one", "auto-policy-turn", "root-one"),
        )
        .unwrap();
    let receipt = gateway
        .process_exact_conversation_submission_for_test(
            &manager,
            &owner,
            "conversation-one",
            "auto-policy-turn",
        )
        .unwrap();
    assert_eq!(
        receipt
            .status
            .finalize_capture
            .as_ref()
            .unwrap()
            .policy_revision,
        2
    );
    gateway
        .set_conversation_policy(
            &manager,
            &owner,
            "conversation-one",
            SequentialChronological,
        )
        .unwrap();
    let replay = gateway
        .process_exact_conversation_submission_for_test(
            &manager,
            &owner,
            "conversation-one",
            "auto-policy-turn",
        )
        .unwrap();
    assert_eq!(replay, receipt);
    gateway.detach_instance(&manager).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}
