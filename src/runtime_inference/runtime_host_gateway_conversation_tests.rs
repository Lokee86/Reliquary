use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, ReliquaryRuntimeRoutes};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

fn gateway() -> GatewayRuntime {
    gateway_at(test_path("conversation-gateway-state"))
}

fn gateway_at(state_dir: PathBuf) -> GatewayRuntime {
    GatewayRuntime::new(
        state_dir,
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

fn seeded_runtime() -> InteractionRuntime {
    seeded_runtime_at(test_path("conversation-test.rel"))
}

fn seeded_runtime_at(path: PathBuf) -> InteractionRuntime {
    let mut cva = Cva::create_project(path).unwrap();
    for (conversation_id, node_id, content) in [
        ("conversation-one", "root-one", "first durable turn"),
        ("conversation-two", "root-two", "second durable turn"),
        ("conversation-three", "root-three", "third durable turn"),
    ] {
        cva.append_node(
            node_id.to_owned(),
            conversation_id.to_owned(),
            None,
            "user".to_owned(),
            1,
            content,
        )
        .unwrap();
        cva.append_branch(Branch {
            id: "main".to_owned(),
            conversation_id: conversation_id.to_owned(),
            leaf_node_id: node_id.to_owned(),
            canonical: true,
        })
        .unwrap();
        if conversation_id == "conversation-one" {
            cva.append_node(
                "root-one-alt".to_owned(),
                conversation_id.to_owned(),
                None,
                "user".to_owned(),
                2,
                "alternate branch root",
            )
            .unwrap();
            cva.append_branch(Branch {
                id: "alternate".to_owned(),
                conversation_id: conversation_id.to_owned(),
                leaf_node_id: "root-one-alt".to_owned(),
                canonical: false,
            })
            .unwrap();
        }
    }
    cva.sync().unwrap();
    InteractionRuntime::new(cva)
}

fn grants(owner_id: &str, capabilities: &[OwnerGrant]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(owner_id.to_owned(), capabilities.iter().copied().collect())])
}

fn submission(conversation_id: &str, id: &str, parent: &str) -> GatewayConversationSubmission {
    GatewayConversationSubmission {
        conversation_id: conversation_id.to_owned(),
        submission_id: id.to_owned(),
        expected_conversation_revision: 0,
        expected_parent_node_id: Some(parent.to_owned()),
        branch_id: "main".to_owned(),
        content: format!("turn {id}"),
        attachments: Vec::new(),
        client_timestamp_ns: None,
    }
}

#[test]
fn rejected_submission_sizes_leave_no_staged_attachment_artifact() {
    let state_dir = test_path("conversation-rejected-payload");
    let gateway = gateway_at(state_dir.clone());
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "size-limit-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();

    let mut oversized_frame = submission("conversation-one", "oversized-frame", "root-one");
    oversized_frame.content = "x".repeat(16 * 1024 * 1024);
    oversized_frame
        .attachments
        .push(GatewayConversationAttachment {
            attachment_id: "small".into(),
            filename: "small.bin".into(),
            mime_type: Some("application/octet-stream".into()),
            bytes: vec![1, 2, 3],
        });
    assert!(
        gateway
            .submit_conversation(&writer, &owner_id, oversized_frame)
            .is_err()
    );
    let rel_staging = state_dir
        .join("conversation-queues")
        .join("staging")
        .join(owner_id.strip_prefix("rel-").unwrap());
    assert!(
        !rel_staging.exists(),
        "an encoded frame rejected at admission must not write attachment blobs"
    );

    let mut stale_revision = submission("conversation-one", "stale-revision", "root-one");
    stale_revision.expected_conversation_revision = 1;
    stale_revision
        .attachments
        .push(GatewayConversationAttachment {
            attachment_id: "stale".into(),
            filename: "stale.bin".into(),
            mime_type: Some("application/octet-stream".into()),
            bytes: vec![4, 5, 6],
        });
    assert!(
        gateway
            .submit_conversation(&writer, &owner_id, stale_revision)
            .is_err()
    );
    assert!(
        !rel_staging.exists(),
        "a stale revision must be rejected before staging attachments"
    );

    let mut oversized_binary = submission("conversation-one", "oversized-binary", "root-one");
    oversized_binary
        .attachments
        .push(GatewayConversationAttachment {
            attachment_id: "too-large".into(),
            filename: "too-large.bin".into(),
            mime_type: Some("application/octet-stream".into()),
            bytes: vec![0; 64 * 1024 * 1024 + 1],
        });
    assert!(
        gateway
            .submit_conversation(&writer, &owner_id, oversized_binary)
            .is_err()
    );
    assert!(
        !rel_staging.exists(),
        "a binary payload above the documented per-attachment bound must be rejected before staging"
    );

    assert!(RelJournal::validate_staged_payload_sizes([64 * 1024 * 1024; 5]).is_err());
    assert!(RelJournal::validate_staged_payload_sizes([0; 65]).is_err());

    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn shared_attachment_blob_is_reclaimed_after_every_terminal_payload_compacts() {
    let state_dir = test_path("conversation-shared-attachment-gc");
    let gateway = gateway_at(state_dir.clone());
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "attachment-gc-principal".into(),
            None,
            grants(
                &owner_id,
                &[
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::CancelOwnPendingSubmission,
                ],
            ),
        )
        .unwrap();

    let bytes = b"shared attachment payload".to_vec();
    for (conversation_id, submission_id, parent) in [
        ("conversation-one", "shared-attachment-one", "root-one"),
        ("conversation-two", "shared-attachment-two", "root-two"),
    ] {
        let mut request = submission(conversation_id, submission_id, parent);
        request.attachments.push(GatewayConversationAttachment {
            attachment_id: "shared-blob".into(),
            filename: "shared.bin".into(),
            mime_type: Some("application/octet-stream".into()),
            bytes: bytes.clone(),
        });
        gateway
            .submit_conversation(&writer, &owner_id, request)
            .unwrap();
    }

    let journal = gateway.conversation.journal(&owner_id).unwrap();
    let first_key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one".to_owned()).unwrap(),
        submission_id: SubmissionId::new("shared-attachment-one".to_owned()).unwrap(),
    };
    let second_key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-two".to_owned()).unwrap(),
        submission_id: SubmissionId::new("shared-attachment-two".to_owned()).unwrap(),
    };
    let hash = {
        let journal = journal.lock().unwrap();
        let first = journal.submission(&first_key).unwrap();
        let second = journal.submission(&second_key).unwrap();
        assert_eq!(
            first.attachments[0].content_hash,
            second.attachments[0].content_hash
        );
        first.attachments[0].content_hash.clone()
    };
    let blob = state_dir
        .join("conversation-queues")
        .join("staging")
        .join(owner_id.strip_prefix("rel-").unwrap())
        .join(format!("{hash}.blob"));
    assert!(blob.exists());

    gateway
        .cancel_conversation_submission(
            &writer,
            &owner_id,
            "conversation-one",
            "shared-attachment-one",
        )
        .unwrap();
    journal.lock().unwrap().compact().unwrap();
    assert!(
        blob.exists(),
        "the second retained submission still references the shared content hash"
    );

    gateway
        .cancel_conversation_submission(
            &writer,
            &owner_id,
            "conversation-two",
            "shared-attachment-two",
        )
        .unwrap();
    journal.lock().unwrap().compact().unwrap();
    assert!(
        !blob.exists(),
        "compaction may reclaim the blob only after no retained payload references it"
    );

    let orphan = journal
        .lock()
        .unwrap()
        .stage_attachment("orphan", "orphan.bin", None, b"orphan bytes")
        .unwrap();
    let orphan_blob = state_dir
        .join("conversation-queues")
        .join("staging")
        .join(owner_id.strip_prefix("rel-").unwrap())
        .join(format!("{}.blob", orphan.content_hash));
    assert!(orphan_blob.exists());
    journal.lock().unwrap().compact().unwrap();
    assert!(
        !orphan_blob.exists(),
        "compaction also removes blobs left unreferenced by a rejected admission"
    );

    drop(journal);
    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn two_instances_share_one_gateway_session_and_viewer_selection_does_not_open_a_session() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "same-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let writer_two = gateway
        .attach_instance(
            "same-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let viewer = gateway
        .attach_instance(
            "viewer-principal".into(),
            None,
            grants(&owner_id, &[OwnerGrant::ViewRel]),
        )
        .unwrap();

    let managed = gateway
        .select_managed_conversation(&viewer, &owner_id, "conversation-two", "main")
        .unwrap();
    assert_eq!(managed.conversation_id.as_deref(), Some("conversation-two"));
    assert!(managed.reopened_session_pending);
    let viewer_token = gateway
        .attach_conversation_viewer(&viewer, &owner_id, "conversation-one")
        .unwrap();
    let view_token = gateway
        .authorize(&viewer, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    gateway
        .with_rel(&view_token, |runtime| {
            assert!(
                runtime.sessions.is_empty(),
                "viewer selection must not create a writable session"
            );
            Ok(())
        })
        .unwrap();
    let writer_session = gateway
        .admit_conversation_writer(&writer, &owner_id, "conversation-one", "main")
        .unwrap();
    assert_eq!(writer_session.leaf_message_id.as_deref(), Some("root-one"));
    let second_writer_session = gateway
        .admit_conversation_writer(&writer_two, &owner_id, "conversation-one", "main")
        .unwrap();
    assert_eq!(second_writer_session, writer_session);
    assert!(
        gateway
            .admit_conversation_writer(&writer_two, &owner_id, "conversation-one", "alternate")
            .is_err()
    );
    let repeated_admission = gateway
        .admit_conversation_writer(&writer, &owner_id, "conversation-one", "main")
        .unwrap();
    assert_eq!(repeated_admission, writer_session);

    gateway
        .with_rel(&view_token, |runtime| {
            assert_eq!(
                runtime.sessions.len(),
                1,
                "viewing an existing writer continuation must not open a duplicate"
            );
            Ok(())
        })
        .unwrap();

    let accepted = gateway
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "turn-one", "root-one"),
        )
        .unwrap();
    assert_eq!(accepted.status.state, SubmissionState::Pending);
    gateway.detach_conversation_viewer(viewer_token).unwrap();
    gateway.detach_instance(&viewer).unwrap();

    let completed = gateway
        .process_next_conversation_submission(&writer, &owner_id, "conversation-one", "turn-one")
        .unwrap();
    assert_eq!(completed.status.state, SubmissionState::Committed);
    let writer_state = gateway.instance_managed_state(&writer, &owner_id).unwrap();
    assert_eq!(
        writer_state.conversation_id.as_deref(),
        Some("conversation-one")
    );

    let token = gateway
        .authorize(&writer, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    gateway
        .with_rel(&token, |runtime| {
            assert_eq!(runtime.sessions.len(), 1);
            assert_eq!(
                runtime.session_leaf_node_id("conversation-one").unwrap(),
                "turn-one",
            );
            assert!(runtime.session("conversation-two").is_none());
            assert!(
                runtime
                    .cva()
                    .archive()
                    .has_node("conversation-one", "turn-one")
            );
            assert!(runtime.cva().archive().branches().iter().any(|branch| {
                branch.id == "main"
                    && branch.conversation_id == "conversation-one"
                    && branch.leaf_node_id == "turn-one"
            }));
            Ok(())
        })
        .unwrap();

    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn cancel_and_finalize_race_has_one_durable_winner_and_survives_remount() {
    let gateway = Arc::new(gateway());
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "race-principal".into(),
            None,
            grants(
                &owner_id,
                &[
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::CancelOwnPendingSubmission,
                ],
            ),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "raced-turn", "root-one"),
        )
        .unwrap();

    let start = Arc::new(Barrier::new(3));
    let process_gateway = Arc::clone(&gateway);
    let process_writer = Arc::clone(&writer);
    let process_owner = owner_id.clone();
    let process_start = Arc::clone(&start);
    let process = std::thread::spawn(move || {
        process_start.wait();
        process_gateway.process_next_conversation_submission(
            &process_writer,
            &process_owner,
            "conversation-one",
            "raced-turn",
        )
    });

    let cancel_gateway = Arc::clone(&gateway);
    let cancel_writer = Arc::clone(&writer);
    let cancel_owner = owner_id.clone();
    let cancel_start = Arc::clone(&start);
    let cancel = std::thread::spawn(move || {
        cancel_start.wait();
        cancel_gateway.cancel_conversation_submission(
            &cancel_writer,
            &cancel_owner,
            "conversation-one",
            "raced-turn",
        )
    });

    start.wait();
    let _ = process.join().unwrap();
    let _ = cancel.join().unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one".to_owned()).unwrap(),
        submission_id: SubmissionId::new("raced-turn".to_owned()).unwrap(),
    };
    let durable_status = gateway
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&key)
        .unwrap();
    assert!(matches!(
        durable_status.state,
        SubmissionState::Committed | SubmissionState::Cancelled
    ));

    let cva = gateway.unmount_rel(&owner_id).unwrap();
    let has_node = cva.archive().has_node("conversation-one", "raced-turn");
    assert_eq!(has_node, durable_status.state == SubmissionState::Committed);
    let remounted = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    assert_eq!(remounted, owner_id);
    let replay_status = gateway
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&key)
        .unwrap();
    assert_eq!(replay_status.state, durable_status.state);

    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn revoking_viewrel_removes_only_that_owners_viewer_registration() {
    let gateway = gateway();
    let owner_a = gateway.mount_rel(seeded_runtime()).unwrap();
    let other_cva = Cva::create_project(test_path("conversation-other-owner.rel")).unwrap();
    let owner_b = gateway
        .mount_rel(InteractionRuntime::new(other_cva))
        .unwrap();
    let viewer = gateway
        .attach_instance(
            "viewer-principal".into(),
            None,
            BTreeMap::from([
                (owner_a.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                (owner_b.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
            ]),
        )
        .unwrap();
    let viewer_a = gateway
        .attach_conversation_viewer(&viewer, &owner_a, "conversation-one")
        .unwrap();
    let viewer_b = gateway
        .attach_conversation_viewer(&viewer, &owner_b, "conversation-one")
        .unwrap();

    gateway
        .replace_instance_grants(
            &viewer,
            BTreeMap::from([(owner_b.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();

    assert!(gateway.detach_conversation_viewer(viewer_a).is_err());
    gateway.detach_conversation_viewer(viewer_b).unwrap();
    gateway.detach_instance(&viewer).unwrap();
    drop(gateway.unmount_rel(&owner_a).unwrap());
    drop(gateway.unmount_rel(&owner_b).unwrap());
}

#[test]
fn fresh_gateway_recovers_finalizing_archive_commit_and_leaves_pending_queued() {
    let state_dir = test_path("conversation-fresh-journal-recovery");
    let archive_path = test_path("conversation-recovery.rel");
    let first = gateway_at(state_dir.clone());
    let owner_id = first
        .mount_rel(seeded_runtime_at(archive_path.clone()))
        .unwrap();
    let writer = first
        .attach_instance(
            "recovery-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();

    first
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "recovered-turn", "root-one"),
        )
        .unwrap();
    first
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-two", "pending-after-restart", "root-two"),
        )
        .unwrap();
    first
        .submit_conversation(
            &writer,
            &owner_id,
            submission(
                "conversation-three",
                "finalize-before-archive",
                "root-three",
            ),
        )
        .unwrap();

    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one".to_owned()).unwrap(),
        submission_id: SubmissionId::new("recovered-turn".to_owned()).unwrap(),
    };
    let capture = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: Some("root-one".to_owned()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "recovery-principal".to_owned(),
        policy_revision: 1,
    };
    let journal = first.conversation.journal(&owner_id).unwrap();
    let submission = {
        let mut journal = journal.lock().unwrap();
        let submission = journal.submission(&key).unwrap().clone();
        let revision = journal.conversation_revision(&key.conversation_id);
        journal
            .claim_finalize(&key, revision, capture.clone())
            .unwrap();
        submission
    };
    let missing_archive_key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-three".to_owned()).unwrap(),
        submission_id: SubmissionId::new("finalize-before-archive".to_owned()).unwrap(),
    };
    let missing_archive_capture = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: Some("root-three".to_owned()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "recovery-principal".to_owned(),
        policy_revision: 1,
    };
    {
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&missing_archive_key.conversation_id);
        journal
            .claim_finalize(&missing_archive_key, revision, missing_archive_capture)
            .unwrap();
    }
    let view_token = first
        .authorize(&writer, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    let archive_receipt = first
        .with_rel(&view_token, |runtime| {
            archive_receipt_or_commit(runtime, &key, &submission, &capture, &[])
        })
        .unwrap();
    assert_eq!(archive_receipt.node_id, "recovered-turn");
    first
        .replace_instance_grants(
            &writer,
            BTreeMap::from([(owner_id.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let cva = first.unmount_rel(&owner_id).unwrap();
    let archive_version_before_recovery = cva.archive_version();
    drop(cva);
    drop(journal);
    first.detach_instance(&writer).unwrap();
    drop(first);

    let recovered = gateway_at(state_dir);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(&archive_path).unwrap()))
            .unwrap(),
        owner_id
    );

    let recovered_finalizing = recovered
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&key)
        .unwrap();
    assert_eq!(recovered_finalizing.state, SubmissionState::Committed);
    assert!(recovered_finalizing.post_commit_receipt.is_some());
    let recovered_before_archive = recovered
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&missing_archive_key)
        .unwrap();
    assert_eq!(recovered_before_archive.state, SubmissionState::Finalizing);
    assert!(recovered_before_archive.post_commit_receipt.is_none());
    let pending_key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-two".to_owned()).unwrap(),
        submission_id: SubmissionId::new("pending-after-restart".to_owned()).unwrap(),
    };
    let pending = recovered
        .conversation
        .journal(&owner_id)
        .unwrap()
        .lock()
        .unwrap()
        .status(&pending_key)
        .unwrap();
    assert_eq!(pending.state, SubmissionState::Pending);

    let recovered_writer = recovered
        .attach_instance(
            "different-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let token = recovered
        .authorize(&recovered_writer, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    recovered
        .with_rel(&token, |runtime| {
            assert!(
                runtime
                    .cva()
                    .archive()
                    .has_node("conversation-one", "recovered-turn")
            );
            assert!(
                !runtime
                    .cva()
                    .archive()
                    .has_node("conversation-three", "finalize-before-archive")
            );
            assert!(
                !runtime
                    .cva()
                    .archive()
                    .has_node("conversation-two", "pending-after-restart")
            );
            let branch = runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|branch| branch.conversation_id == "conversation-one" && branch.id == "main")
                .unwrap();
            assert_eq!(branch.leaf_node_id, "recovered-turn");
            assert_eq!(
                runtime.cva().archive_version(),
                archive_version_before_recovery,
                "startup reconciles the published node but holds a missing-node Finalizing entry"
            );
            Ok(())
        })
        .unwrap();

    // Any currently authorized processor resumes the original admitted identity.
    let resumed = recovered
        .process_next_conversation_submission(
            &recovered_writer,
            &owner_id,
            "conversation-three",
            "finalize-before-archive",
        )
        .unwrap();
    assert_eq!(resumed.status.state, SubmissionState::Committed);
    assert!(resumed.status.post_commit_receipt.is_some());
    let archive_version_after_resume = {
        let token = recovered
            .authorize(&recovered_writer, &owner_id, OwnerGrant::ViewRel)
            .unwrap();
        recovered
            .with_rel(&token, |runtime| {
                assert!(
                    runtime
                        .cva()
                        .archive()
                        .has_node("conversation-three", "finalize-before-archive")
                );
                Ok(runtime.cva().archive_version())
            })
            .unwrap()
    };
    assert_eq!(
        archive_version_after_resume,
        archive_version_before_recovery + 2
    );

    // Terminal retries after restart are receipt lookups, not new Archive publications.
    let retry = recovered
        .process_next_conversation_submission(
            &recovered_writer,
            &owner_id,
            "conversation-one",
            "recovered-turn",
        )
        .unwrap();
    assert_eq!(retry.status.state, SubmissionState::Committed);
    let missing_retry = recovered
        .process_next_conversation_submission(
            &recovered_writer,
            &owner_id,
            "conversation-three",
            "finalize-before-archive",
        )
        .unwrap();
    assert_eq!(missing_retry.status.state, SubmissionState::Committed);
    let archive_version_after_retries = {
        let token = recovered
            .authorize(&recovered_writer, &owner_id, OwnerGrant::ViewRel)
            .unwrap();
        recovered
            .with_rel(&token, |runtime| Ok(runtime.cva().archive_version()))
            .unwrap()
    };
    assert_eq!(archive_version_after_retries, archive_version_after_resume);
    recovered.detach_instance(&recovered_writer).unwrap();
    drop(recovered.unmount_rel(&owner_id).unwrap());
}

#[test]
fn shared_cancel_grant_takes_precedence_over_own_only_when_both_are_present() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let submitter = gateway
        .attach_instance(
            "submitter".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let own_only = gateway
        .attach_instance(
            "own-only".into(),
            None,
            grants(&owner_id, &[OwnerGrant::CancelOwnPendingSubmission]),
        )
        .unwrap();
    let shared = gateway
        .attach_instance(
            "shared-canceler".into(),
            None,
            grants(
                &owner_id,
                &[
                    OwnerGrant::CancelOwnPendingSubmission,
                    OwnerGrant::CancelSharedGeneration,
                ],
            ),
        )
        .unwrap();

    gateway
        .submit_conversation(
            &submitter,
            &owner_id,
            submission("conversation-one", "foreign-shared", "root-one"),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &submitter,
            &owner_id,
            submission("conversation-two", "foreign-own-only", "root-two"),
        )
        .unwrap();

    assert!(
        gateway
            .cancel_conversation_submission(
                &own_only,
                &owner_id,
                "conversation-two",
                "foreign-own-only",
            )
            .is_err()
    );
    let cancelled = gateway
        .cancel_conversation_submission(&shared, &owner_id, "conversation-one", "foreign-shared")
        .unwrap();
    assert_eq!(cancelled.status.state, SubmissionState::Cancelled);

    gateway.detach_instance(&submitter).unwrap();
    gateway.detach_instance(&own_only).unwrap();
    gateway.detach_instance(&shared).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn journal_compaction_threshold_seam_compacts_without_a_large_fixture() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let journal = gateway.conversation.journal(&owner_id).unwrap();
    {
        let mut journal = journal.lock().unwrap();
        assert!(!journal.needs_compaction_at(u64::MAX).unwrap());
        assert!(journal.needs_compaction_at(1).unwrap());
        journal.compact().unwrap();
        assert!(journal.statuses().is_empty());
        assert!(!journal.needs_compaction_at(u64::MAX).unwrap());
    }
    drop(journal);
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn accepted_turn_outlives_detach_and_keeps_its_admitted_principal() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "original-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    gateway
        .admit_conversation_writer(&writer, &owner_id, "conversation-one", "main")
        .unwrap();
    let viewer = gateway
        .attach_conversation_viewer(&writer, &owner_id, "conversation-one")
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "detached-turn", "root-one"),
        )
        .unwrap();
    gateway.detach_instance(&writer).unwrap();
    assert!(
        gateway
            .conversation
            .viewers
            .lock()
            .unwrap()
            .viewers
            .is_empty()
    );
    assert!(gateway.detach_conversation_viewer(viewer).is_err());
    assert!(
        gateway
            .process_next_conversation_submission(
                &writer,
                &owner_id,
                "conversation-one",
                "detached-turn"
            )
            .is_err()
    );
    let helper = gateway
        .attach_instance(
            "helper-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let receipt = gateway
        .process_next_conversation_submission(
            &helper,
            &owner_id,
            "conversation-one",
            "detached-turn",
        )
        .unwrap();
    assert_eq!(receipt.status.state, SubmissionState::Committed);
    assert_eq!(
        receipt.status.archive_receipt.unwrap().principal_id,
        "original-principal"
    );
    let token = gateway
        .authorize(&helper, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    gateway
        .with_rel(&token, |runtime| {
            assert_eq!(
                runtime
                    .session("conversation-one")
                    .unwrap()
                    .leaf_message_id
                    .as_deref(),
                Some("detached-turn")
            );
            assert_eq!(
                runtime
                    .cva()
                    .archive()
                    .nodes
                    .get("conversation-one", "detached-turn")
                    .unwrap()
                    .principal_id
                    .as_deref(),
                Some("original-principal")
            );
            Ok(())
        })
        .unwrap();
    gateway.detach_instance(&helper).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn cancellation_after_submitter_detach_requires_current_applicable_grant() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "submitter".into(),
            None,
            grants(
                &owner_id,
                &[
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::CancelOwnPendingSubmission,
                ],
            ),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "detached-cancel", "root-one"),
        )
        .unwrap();
    let stale = gateway
        .authorize(&writer, &owner_id, OwnerGrant::CancelOwnPendingSubmission)
        .unwrap();
    gateway.detach_instance(&writer).unwrap();
    assert!(with_admission(&gateway, &stale, || Ok(())).is_err());
    assert!(
        gateway
            .cancel_conversation_submission(
                &writer,
                &owner_id,
                "conversation-one",
                "detached-cancel"
            )
            .is_err()
    );
    let own_only = gateway
        .attach_instance(
            "other-principal".into(),
            None,
            grants(&owner_id, &[OwnerGrant::CancelOwnPendingSubmission]),
        )
        .unwrap();
    assert!(
        gateway
            .cancel_conversation_submission(
                &own_only,
                &owner_id,
                "conversation-one",
                "detached-cancel"
            )
            .is_err()
    );
    let shared = gateway
        .attach_instance(
            "shared-canceler".into(),
            None,
            grants(&owner_id, &[OwnerGrant::CancelSharedGeneration]),
        )
        .unwrap();
    let stale_grant = gateway
        .authorize(&shared, &owner_id, OwnerGrant::CancelSharedGeneration)
        .unwrap();
    gateway
        .replace_instance_grants(&shared, BTreeMap::new())
        .unwrap();
    gateway
        .replace_instance_grants(
            &shared,
            grants(&owner_id, &[OwnerGrant::CancelSharedGeneration]),
        )
        .unwrap();
    assert!(with_admission(&gateway, &stale_grant, || Ok(())).is_err());
    let cancelled = gateway
        .cancel_conversation_submission(&shared, &owner_id, "conversation-one", "detached-cancel")
        .unwrap();
    assert_eq!(cancelled.status.state, SubmissionState::Cancelled);
    assert_eq!(
        gateway
            .process_accepted_conversation_submission(
                &owner_id,
                "conversation-one",
                "detached-cancel"
            )
            .unwrap()
            .status
            .state,
        SubmissionState::Cancelled
    );
    gateway.detach_instance(&own_only).unwrap();
    gateway.detach_instance(&shared).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn accepted_queue_survives_restart_and_runs_without_any_attachment() {
    let state_dir = test_path("f2-owner-coordinator-state");
    let archive_path = test_path("f2-owner-coordinator.rel");
    let first = gateway_at(state_dir.clone());
    let owner_id = first
        .mount_rel(seeded_runtime_at(archive_path.clone()))
        .unwrap();
    let writer = first
        .attach_instance(
            "durable-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let mut request = submission("conversation-one", "no-viewer-restart", "root-one");
    request.attachments.push(GatewayConversationAttachment {
        attachment_id: "payload".into(),
        filename: "payload.bin".into(),
        mime_type: None,
        bytes: vec![1, 3, 5],
    });
    first
        .submit_conversation(&writer, &owner_id, request)
        .unwrap();
    first
        .replace_instance_grants(&writer, BTreeMap::new())
        .unwrap();
    first.detach_instance(&writer).unwrap();
    drop(first.unmount_rel(&owner_id).unwrap());
    drop(first);
    let recovered = gateway_at(state_dir);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(&archive_path).unwrap()))
            .unwrap(),
        owner_id
    );
    assert!(recovered.instances.read().unwrap().is_empty());
    let receipt = recovered
        .process_accepted_conversation_submission(
            &owner_id,
            "conversation-one",
            "no-viewer-restart",
        )
        .unwrap();
    assert_eq!(receipt.status.state, SubmissionState::Committed);
    let archive = receipt.status.archive_receipt.unwrap();
    assert_eq!(archive.principal_id, "durable-principal");
    assert_eq!(archive.attachment_identities.len(), 1);
    assert!(receipt.status.post_commit_receipt.is_some());
    drop(recovered.unmount_rel(&owner_id).unwrap());
}

#[test]
fn accepted_finalization_rejects_changed_identity_and_stale_owner_fence() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "admitted-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner_id,
            submission("conversation-one", "identity-check", "root-one"),
        )
        .unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("identity-check").unwrap(),
    };
    let journal = gateway.conversation.journal(&owner_id).unwrap();
    let admitted = journal.lock().unwrap().submission(&key).unwrap().clone();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "replacement-principal".into(),
        policy_revision: 1,
    };
    let authority = OperationAuthority::capture(&gateway, &owner_id).unwrap();
    assert!(
        with_operation(&gateway, &authority, |runtime| archive_receipt_or_commit(
            runtime,
            &key,
            &admitted,
            &capture,
            &[]
        ))
        .is_err()
    );
    with_operation(&gateway, &authority, |runtime| {
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "identity-check")
        );
        Ok(())
    })
    .unwrap();
    gateway.quiesce_rel(&owner_id).unwrap();
    assert!(with_accepted_work(&gateway, &authority, || Ok(())).is_err());
    gateway.reopen_rel(&owner_id).unwrap();
    assert!(with_accepted_work(&gateway, &authority, || Ok(())).is_err());
    gateway.detach_instance(&writer).unwrap();
    drop(journal);
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn earlier_admission_retries_preserve_original_authority_and_fingerprint() {
    let gateway = gateway();
    let owner_id = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "earlier-principal".into(),
            None,
            grants(
                &owner_id,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let request = submission("conversation-one", "earlier-admission", "root-one");
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("earlier-admission").unwrap(),
    };
    let earlier = Submission {
        id: key.clone(),
        principal_id: "earlier-principal".into(),
        scope_fingerprint: fingerprint(&json!({
            "rel_id": owner_id, "conversation_id": "conversation-one", "branch_id": "main",
        }))
        .unwrap(),
        payload_hash: Submission::canonical_payload_hash(&request.content, &[]).unwrap(),
        request_metadata: json!({
            "branch_id": "main", "expected_parent_node_id": "root-one",
        }),
        content: request.content.clone(),
        attachments: vec![],
        client_timestamp_ns: None,
    };
    let journal = gateway.conversation.journal(&owner_id).unwrap();
    let admitted = journal
        .lock()
        .unwrap()
        .enqueue_with_tie_key(earlier.clone(), now_ns(), 0, [1; 16])
        .unwrap();
    let admitted_sequence = match admitted {
        EnqueueOutcome::Durable(status) | EnqueueOutcome::Idempotent(status) => {
            status.admission_sequence
        }
    };
    let retry = gateway
        .submit_conversation(&writer, &owner_id, request.clone())
        .unwrap();
    assert_eq!(retry.status.admission_sequence, admitted_sequence);
    assert_eq!(journal.lock().unwrap().submission(&key), Some(&earlier));
    let mut changed = request;
    changed.content.push_str(" changed");
    assert!(
        gateway
            .submit_conversation(&writer, &owner_id, changed)
            .is_err()
    );
    gateway.detach_instance(&writer).unwrap();
    let committed = gateway
        .process_accepted_conversation_submission(
            &owner_id,
            "conversation-one",
            "earlier-admission",
        )
        .unwrap();
    assert_eq!(
        committed.status.archive_receipt.unwrap().principal_id,
        "earlier-principal"
    );
    drop(journal);
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn accepted_queue_freezes_during_quiesce_without_blocking_another_rel() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let other = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "freeze-principal".into(),
            None,
            BTreeMap::from([
                (
                    owner.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]),
                ),
                (
                    other.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]),
                ),
            ]),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "frozen", "root-one"),
        )
        .unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("frozen").unwrap(),
    };
    let journal = gateway.conversation.journal(&owner).unwrap();
    let admitted = journal.lock().unwrap().submission(&key).unwrap().clone();
    let before = journal.lock().unwrap().status(&key).unwrap();
    let old = OperationAuthority::capture(&gateway, &owner).unwrap();
    gateway.quiesce_rel(&owner).unwrap();
    assert!(
        gateway
            .process_accepted_conversation_submission(&owner, "conversation-one", "frozen")
            .is_err()
    );
    assert!(
        gateway
            .submit_conversation(
                &writer,
                &owner,
                submission("conversation-two", "closed-admission", "root-two")
            )
            .is_err()
    );
    assert_eq!(journal.lock().unwrap().submission(&key), Some(&admitted));
    assert_eq!(journal.lock().unwrap().status(&key).unwrap(), before);
    gateway
        .with_quiesced_rel(&owner, |runtime| {
            assert!(
                !runtime
                    .cva()
                    .archive()
                    .has_node("conversation-one", "frozen")
            );
            Ok(())
        })
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &other,
            submission("conversation-one", "unrelated", "root-one"),
        )
        .unwrap();
    assert_eq!(
        gateway
            .process_next_conversation_submission(&writer, &other, "conversation-one", "unrelated")
            .unwrap()
            .status
            .state,
        SubmissionState::Committed
    );
    gateway.reopen_rel(&owner).unwrap();
    assert!(with_accepted_work(&gateway, &old, || Ok(())).is_err());
    let result = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one", "frozen")
        .unwrap();
    assert_eq!(result.status.state, SubmissionState::Committed);
    assert_eq!(
        result.status.archive_receipt.unwrap().principal_id,
        "freeze-principal"
    );
    gateway.detach_instance(&writer).unwrap();
    drop(journal);
    drop(gateway.unmount_rel(&owner).unwrap());
    drop(gateway.unmount_rel(&other).unwrap());
}

#[test]
fn reconcile_failure_keeps_owner_closed_and_workers_stopped() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "reconcile-principal".into(),
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
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "conflicting-node", "root-one"),
        )
        .unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("conflicting-node").unwrap(),
    };
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "reconcile-principal".into(),
        policy_revision: 1,
    };
    let journal = gateway.conversation.journal(&owner).unwrap();
    {
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&key.conversation_id);
        journal
            .claim_finalize(&key, revision, capture.clone())
            .unwrap();
    }
    let token = gateway
        .authorize(&writer, &owner, OwnerGrant::ManageRel)
        .unwrap();
    gateway
        .with_rel(&token, |runtime| {
            // An unrelated publication reused the stable ID but is not this admission.
            runtime
                .cva
                .append_node(
                    "conflicting-node".into(),
                    "conversation-one".into(),
                    Some("root-one".into()),
                    "user".into(),
                    capture.committed_timestamp_ns,
                    "content that does not match admission",
                )
                .map_err(operation)?;
            runtime.cva.sync().map_err(operation)
        })
        .unwrap();
    gateway.quiesce_rel(&owner).unwrap();
    for client_path in [false, true] {
        let result = if client_path {
            gateway.reopen_rel_for(&writer, &owner)
        } else {
            gateway.reopen_rel(&owner)
        };
        assert!(result.is_err());
        assert_eq!(
            gateway.owner_admission(&owner).unwrap(),
            super::super::OwnerAdmission::Closed
        );
        assert!(
            gateway
                .rel_executions
                .read()
                .unwrap()
                .get(&owner)
                .unwrap()
                .owner
                .lock()
                .unwrap()
                .workers_stopped()
        );
        assert!(
            gateway
                .authorize(&writer, &owner, OwnerGrant::ViewRel)
                .is_err()
        );
        assert_eq!(
            journal.lock().unwrap().status(&key).unwrap().state,
            SubmissionState::Finalizing
        );
    }
    gateway.detach_instance(&writer).unwrap();
    drop(journal);
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn accepted_owner_authority_cannot_publish_after_remount() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = gateway
        .attach_instance(
            "remount-principal".into(),
            None,
            grants(
                &owner,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "after-remount", "root-one"),
        )
        .unwrap();
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("after-remount").unwrap(),
    };
    let admitted = gateway
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .submission(&key)
        .unwrap()
        .clone();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: "remount-principal".into(),
        policy_revision: 1,
    };
    let old = OperationAuthority::capture(&gateway, &owner).unwrap();
    let cva = gateway.unmount_rel(&owner).unwrap();
    assert_eq!(
        gateway.mount_rel(InteractionRuntime::new(cva)).unwrap(),
        owner
    );
    assert!(
        with_operation(&gateway, &old, |runtime| archive_receipt_or_commit(
            runtime,
            &key,
            &admitted,
            &capture,
            &[]
        ))
        .is_err()
    );
    let current = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &current, |runtime| {
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "after-remount")
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        gateway
            .process_next_conversation_submission(
                &writer,
                &owner,
                "conversation-one",
                "after-remount"
            )
            .unwrap()
            .status
            .state,
        SubmissionState::Committed
    );
    gateway.detach_instance(&writer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}
