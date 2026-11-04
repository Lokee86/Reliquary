use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!("queue-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}
fn submission(text: &str) -> QueuedSubmission {
    QueuedSubmission::new(
        ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
            .unwrap(),
        "alice",
        text,
    )
    .unwrap()
}

#[test]
fn queue_is_owner_bound_durable_and_replays_ordered_pending_work() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([17; 16]);
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission("hello"), 0).unwrap();
        assert_eq!(queue.pending_in_order("conversation").len(), 1);
    }
    let wrong_owner = OwnerId::from_bytes([18; 16]);
    let original = dir
        .join("conversation-queues")
        .join(format!("{}.rqj", uuid_filename(owner.as_bytes())));
    let mismatched = dir
        .join("conversation-queues")
        .join(format!("{}.rqj", uuid_filename(wrong_owner.as_bytes())));
    fs::copy(original, mismatched).unwrap();
    assert!(matches!(
        ConversationQueue::open(&dir, wrong_owner),
        Err(QueueError::OwnerMismatch)
    ));
    let queue = ConversationQueue::open(&dir, owner).unwrap();
    assert_eq!(queue.sequence(), 1);
    assert_eq!(queue.pending_in_order("conversation").len(), 1);
    drop(queue);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancel_and_finalize_have_one_durable_winner_and_finalize_needs_receipt_proof() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([19; 16]);
    let mut queue = ConversationQueue::open(&dir, owner).unwrap();
    let submission = submission("hello");
    let id = submission.id.clone();
    queue.enqueue(submission.clone(), 0).unwrap();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: None,
        committed_timestamp_ns: 7,
        verified_principal_id: "alice".into(),
    };
    let finalized = queue.claim_finalize(&id, 1, capture.clone()).unwrap();
    assert_eq!(finalized.state, QueueState::Finalizing);
    assert!(matches!(
        queue.cancel(&id, 1),
        Err(QueueError::InvalidState)
    ));
    assert_eq!(queue.claim_finalize(&id, 1, capture).unwrap(), finalized);
    assert!(queue.status(&id).unwrap().archive_receipt.is_none());

    let cancelled_submission = QueuedSubmission::new(
        ConversationSubmissionId::new(
            "conversation",
            SubmissionId::new("cancelled-submission").unwrap(),
        )
        .unwrap(),
        "alice",
        "cancel me",
    )
    .unwrap();
    let cancelled_id = cancelled_submission.id.clone();
    queue.enqueue(cancelled_submission, 2).unwrap();
    assert_eq!(
        queue.cancel(&cancelled_id, 3).unwrap().state,
        QueueState::Cancelled
    );
    assert!(matches!(
        queue.claim_finalize(
            &cancelled_id,
            4,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 8,
                verified_principal_id: "alice".into(),
            }
        ),
        Err(QueueError::InvalidState)
    ));

    let wrong = ArchiveCommitProof(ArchiveReceipt {
        node_id: "other-node".into(),
        branch_id: "main".into(),
        parent_node_id: None,
        principal_id: "alice".into(),
        committed_timestamp_ns: 7,
        payload_hash: submission.payload_hash,
    });
    assert!(matches!(
        queue.record_archive_commit(&id, wrong),
        Err(QueueError::ArchiveIdentityConflict)
    ));
    assert_eq!(queue.status(&id).unwrap().state, QueueState::Finalizing);
    drop(queue);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancel_and_finalize_winners_remain_unambiguous_after_queue_reopen() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([29; 16]);
    let mut queue = ConversationQueue::open(&dir, owner).unwrap();
    let finalizing = submission("finalize across restart");
    let finalizing_id = finalizing.id.clone();
    queue.enqueue(finalizing, 0).unwrap();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        parent_node_id: None,
        committed_timestamp_ns: 29,
        verified_principal_id: "alice".into(),
    };
    let won_finalize = queue
        .claim_finalize(&finalizing_id, 1, capture.clone())
        .unwrap();

    let cancelled = QueuedSubmission::new(
        ConversationSubmissionId::new(
            "conversation",
            SubmissionId::new("cancel-after-reopen").unwrap(),
        )
        .unwrap(),
        "alice",
        "cancel across restart",
    )
    .unwrap();
    let cancelled_id = cancelled.id.clone();
    queue.enqueue(cancelled, 2).unwrap();
    drop(queue);

    let mut recovered = ConversationQueue::open(&dir, owner).unwrap();
    assert_eq!(recovered.status(&finalizing_id), Some(won_finalize.clone()));
    assert_eq!(
        recovered
            .claim_finalize(&finalizing_id, 2, capture.clone())
            .unwrap(),
        won_finalize
    );
    assert!(matches!(
        recovered.cancel(&finalizing_id, 2),
        Err(QueueError::InvalidState)
    ));
    let won_cancel = recovered.cancel(&cancelled_id, 3).unwrap();
    assert_eq!(won_cancel.state, QueueState::Cancelled);
    drop(recovered);

    let mut recovered_again = ConversationQueue::open(&dir, owner).unwrap();
    assert_eq!(
        recovered_again.status(&finalizing_id).unwrap().state,
        QueueState::Finalizing
    );
    assert!(
        recovered_again
            .status(&finalizing_id)
            .unwrap()
            .archive_receipt
            .is_none()
    );
    assert!(matches!(
        recovered_again.claim_finalize(
            &finalizing_id,
            3,
            FinalizeCapture {
                branch_id: "fork".into(),
                ..capture
            }
        ),
        Err(QueueError::ArchiveIdentityConflict)
    ));
    assert_eq!(
        recovered_again.cancel(&cancelled_id, 0).unwrap(),
        won_cancel
    );
    assert!(matches!(
        recovered_again.claim_finalize(
            &cancelled_id,
            4,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 30,
                verified_principal_id: "alice".into(),
            }
        ),
        Err(QueueError::InvalidState)
    ));
    drop(recovered_again);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn duplicate_enqueue_requires_identical_payload_and_revision() {
    let dir = temp_dir();
    let mut queue = ConversationQueue::open(&dir, OwnerId::from_bytes([20; 16])).unwrap();
    queue.enqueue(submission("first"), 0).unwrap();
    assert!(matches!(
        queue.enqueue(submission("different"), 1),
        Err(QueueError::DuplicatePayloadConflict)
    ));
    assert!(matches!(
        queue.enqueue(
            QueuedSubmission::new(
                ConversationSubmissionId::new("conversation", SubmissionId::new("other").unwrap())
                    .unwrap(),
                "alice",
                "second"
            )
            .unwrap(),
            0
        ),
        Err(QueueError::StaleRevision { .. })
    ));
    drop(queue);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn recovery_discards_only_an_incomplete_final_frame() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([21; 16]);
    let path;
    let valid_len;
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission("durable"), 0).unwrap();
        path = queue.path().to_path_buf();
        valid_len = fs::metadata(&path).unwrap().len();
    }
    {
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&[12, 0]).unwrap();
        file.sync_all().unwrap();
    }
    {
        let queue = ConversationQueue::open(&dir, owner).unwrap();
        assert_eq!(queue.sequence(), 1);
        assert_eq!(fs::metadata(&path).unwrap().len(), valid_len);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn recovery_rejects_a_corrupt_complete_frame() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([22; 16]);
    let path;
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission("durable"), 0).unwrap();
        path = queue.path().to_path_buf();
    }
    {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        file.seek(SeekFrom::Start(len - 1)).unwrap();
        let mut byte = [0];
        file.read_exact(&mut byte).unwrap();
        byte[0] ^= 0xff;
        file.seek(SeekFrom::Start(len - 1)).unwrap();
        file.write_all(&byte).unwrap();
        file.sync_all().unwrap();
    }
    assert!(matches!(
        ConversationQueue::open(&dir, owner),
        Err(QueueError::Corrupt("complete frame checksum"))
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_host_status_is_durable_held_and_never_recreates_dispatch_intent() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([24; 16]);
    let stable_id = "op-24".to_owned();
    let digest = [9; 32];
    let submission = submission("host work");
    let id = submission.id.clone();
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission, 0).unwrap();
        assert!(
            queue
                .begin_host_handoff(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    stable_id.clone(),
                    digest
                )
                .unwrap()
        );
    }
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        // Existing intent means status reconciliation, never redispatch.
        assert!(
            !queue
                .begin_host_handoff(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    stable_id.clone(),
                    digest
                )
                .unwrap()
        );
        let held = queue
            .record_host_status(
                &id,
                RuntimeCapability::ConversationExecution,
                HostOperationStatus::Unknown,
            )
            .unwrap();
        let operation = held
            .host_operations
            .get(&RuntimeCapability::ConversationExecution)
            .unwrap();
        assert_eq!(operation.phase, DurableHostOperationPhase::Held);
        assert_eq!(
            operation.last_known_phase,
            Some(DurableHostOperationPhase::HandoffIntent)
        );
        assert!(operation.held());
        assert!(operation.requires_reconciliation());
        drop(queue);
    }
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        let operation = queue
            .host_operation(&id, RuntimeCapability::ConversationExecution)
            .unwrap();
        assert_eq!(operation.phase, DurableHostOperationPhase::Held);
        assert_eq!(
            operation.last_known_phase,
            Some(DurableHostOperationPhase::HandoffIntent)
        );
        assert!(operation.held());
        assert!(operation.requires_reconciliation());
        assert!(
            !queue
                .begin_host_handoff(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    stable_id.clone(),
                    digest
                )
                .unwrap()
        );
        assert!(queue.pending_in_order("conversation").is_empty());
        assert!(matches!(
            queue.claim_finalize(
                &id,
                1,
                FinalizeCapture {
                    branch_id: "main".into(),
                    parent_node_id: None,
                    committed_timestamp_ns: 9,
                    verified_principal_id: "alice".into(),
                }
            ),
            Err(QueueError::InvalidState)
        ));

        let pending = queue
            .record_host_status(
                &id,
                RuntimeCapability::ConversationExecution,
                HostOperationStatus::Pending,
            )
            .unwrap();
        let operation = pending
            .host_operations
            .get(&RuntimeCapability::ConversationExecution)
            .unwrap();
        assert_eq!(operation.phase, DurableHostOperationPhase::Pending);
        assert!(!operation.held());
        // Even a known in-flight operation is not scheduled for a second handoff.
        assert!(queue.pending_in_order("conversation").is_empty());
        let completed = queue
            .record_host_status(
                &id,
                RuntimeCapability::ConversationExecution,
                HostOperationStatus::Completed,
            )
            .unwrap();
        assert_eq!(
            completed.host_operations[&RuntimeCapability::ConversationExecution].phase,
            DurableHostOperationPhase::Completed
        );
        assert!(
            queue
                .claim_finalize(
                    &id,
                    1,
                    FinalizeCapture {
                        branch_id: "main".into(),
                        parent_node_id: None,
                        committed_timestamp_ns: 9,
                        verified_principal_id: "alice".into(),
                    }
                )
                .is_ok()
        );
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_host_cancel_keeps_cancel_intent_and_cancel_requires_terminal_proof() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([25; 16]);
    let mut queue = ConversationQueue::open(&dir, owner).unwrap();
    let submission = submission("cancel host work");
    let id = submission.id.clone();
    queue.enqueue(submission, 0).unwrap();
    queue
        .begin_host_handoff(
            &id,
            RuntimeCapability::ConversationExecution,
            "op-25".into(),
            [8; 32],
        )
        .unwrap();
    queue
        .begin_host_cancel(&id, RuntimeCapability::ConversationExecution)
        .unwrap();
    let held = queue
        .record_host_cancel(
            &id,
            RuntimeCapability::ConversationExecution,
            HostCancelOutcome::Unknown,
        )
        .unwrap();
    let operation = held
        .host_operations
        .get(&RuntimeCapability::ConversationExecution)
        .unwrap();
    assert_eq!(operation.phase, DurableHostOperationPhase::Held);
    assert!(operation.cancel_requested);
    assert_eq!(
        operation.last_known_phase,
        Some(DurableHostOperationPhase::CancelIntent)
    );
    assert!(matches!(
        queue.cancel(&id, 1),
        Err(QueueError::InvalidState)
    ));
    let cancelled = queue
        .record_host_cancel(
            &id,
            RuntimeCapability::ConversationExecution,
            HostCancelOutcome::CancelledBeforeStart,
        )
        .unwrap();
    assert_eq!(
        cancelled.host_operations[&RuntimeCapability::ConversationExecution].phase,
        DurableHostOperationPhase::Cancelled
    );
    assert_eq!(queue.cancel(&id, 1).unwrap().state, QueueState::Cancelled);
    drop(queue);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn generation_checkpoints_are_synced_recovered_and_replayed_with_bounds() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([26; 16]);
    let submission = submission("generate");
    let id = submission.id.clone();
    {
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission, 0).unwrap();
        queue
            .begin_generation(
                &id,
                "op-26".into(),
                "gen-26".into(),
                "main".into(),
                "parent".into(),
                10,
            )
            .unwrap();
        for sequence in 1u64..=70 {
            let prefix = "x".repeat(sequence as usize);
            queue
                .record_generation_checkpoint(&id, sequence, prefix)
                .unwrap();
        }
        let before_idempotent = queue.sequence();
        queue
            .record_generation_checkpoint(&id, 70, "x".repeat(70))
            .unwrap();
        assert_eq!(queue.sequence(), before_idempotent);
        assert!(matches!(
            queue.replay_generation_checkpoints(&id, 0, 10).unwrap(),
            GenerationReplay::ResyncRequired { latest_cursor: 70 }
        ));
    }
    {
        let queue = ConversationQueue::open(&dir, owner).unwrap();
        let latest = queue.generation_checkpoint(&id).unwrap();
        assert_eq!(latest.sequence, 70);
        assert_eq!(latest.content_prefix, "x".repeat(70));
        assert_eq!(latest.utf8_bytes, 70);
        assert_eq!(
            queue.replay_generation_checkpoints(&id, 69, 1).unwrap(),
            GenerationReplay::Checkpoints {
                latest_cursor: 70,
                items: vec![latest]
            }
        );
        assert!(matches!(
            queue.replay_generation_checkpoints(&id, 70, 0),
            Err(QueueError::InvalidReplayBound)
        ));
        assert!(matches!(
            queue.replay_generation_checkpoints(&id, 71, 1),
            Err(QueueError::InvalidReplayCursor)
        ));
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_checkpoint_append_never_becomes_visible_before_sync() {
    let dir = temp_dir();
    let owner = OwnerId::from_bytes([27; 16]);
    let submission = submission("generate");
    let id = submission.id.clone();
    let mut queue = ConversationQueue::open(&dir, owner).unwrap();
    queue.enqueue(submission, 0).unwrap();
    queue
        .begin_generation(
            &id,
            "op-27".into(),
            "gen-27".into(),
            "main".into(),
            "parent".into(),
            11,
        )
        .unwrap();
    let path = queue.path().to_path_buf();
    queue.file = OpenOptions::new().read(true).open(&path).unwrap();
    assert!(matches!(
        queue.record_generation_checkpoint(&id, 1, "uncommitted output".into()),
        Err(QueueError::Io(_))
    ));
    assert!(queue.generation_checkpoint(&id).is_none());
    drop(queue);
    let queue = ConversationQueue::open(&dir, owner).unwrap();
    assert!(queue.generation_checkpoint(&id).is_none());
    drop(queue);
    fs::remove_dir_all(dir).unwrap();
}
