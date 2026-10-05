use super::*;
use crate::HandoffReceipt;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{Arc, Barrier, Mutex},
    thread,
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("reliquary-journal-{}", Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn id(s: &str) -> ConversationId {
    ConversationId::new(s).unwrap()
}
fn sid(s: &str) -> SubmissionId {
    SubmissionId::new(s).unwrap()
}
fn key(c: &str, s: &str) -> ConversationSubmissionId {
    ConversationSubmissionId {
        conversation_id: id(c),
        submission_id: sid(s),
    }
}
fn submission(c: &str, s: &str, body: &str) -> Submission {
    let content = body.to_owned();
    let attachments = vec![];
    let payload_hash = Submission::canonical_payload_hash(&content, &attachments).unwrap();
    Submission {
        id: key(c, s),
        principal_id: "principal-1".into(),
        scope_fingerprint: "scope-A".into(),
        payload_hash,
        request_metadata: serde_json::json!({"source":"test","revision":1}),
        content,
        attachments,
        client_timestamp_ns: Some(-999),
    }
}
fn enqueue(
    j: &mut RelJournal,
    s: Submission,
    wall_ns: i64,
) -> Result<EnqueueOutcome, JournalError> {
    let revision = j.conversation_revision(&s.id.conversation_id);
    j.enqueue(s, wall_ns, revision)
}
fn open(t: &Temp, uuid: Uuid) -> RelJournal {
    RelJournal::open(t.path(), RelOwnerId::from_uuid(uuid)).unwrap()
}
fn capture() -> FinalizeCapture {
    FinalizeCapture {
        branch_id: "branch-1".into(),
        branch_label: None,
        parent_node_id: Some("parent-0".into()),
        committed_timestamp_ns: 123,
        verified_principal_id: "principal-1".into(),
        policy_revision: 7,
    }
}
fn receipt(s: &Submission, c: &FinalizeCapture) -> ArchiveReceipt {
    ArchiveReceipt {
        node_id: s.id.submission_id.as_str().into(),
        branch_id: c.branch_id.clone(),
        parent_node_id: c.parent_node_id.clone(),
        principal_id: c.verified_principal_id.clone(),
        committed_timestamp_ns: c.committed_timestamp_ns,
        payload_hash: s.payload_hash.clone(),
        attachment_identities: s.attachments.clone(),
    }
}

#[test]
fn rel_journal_scopes_ids_and_sequences_all_conversations_and_recovers_cancel_tombstones() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut j = open(&t, rel);
    let a = submission("conversation-a", "same", "a");
    let b = submission("conversation-b", "same", "b");
    assert!(matches!(
        enqueue(&mut j, a.clone(), 100),
        Ok(EnqueueOutcome::Durable(_))
    ));
    assert!(matches!(
        enqueue(&mut j, b.clone(), 99),
        Ok(EnqueueOutcome::Durable(_))
    ));
    assert_eq!(j.sequence(), 2);
    assert_eq!(j.status(&a.id).unwrap().verified_receive_ns, 100);
    assert_eq!(j.status(&b.id).unwrap().verified_receive_ns, 100);
    assert!(matches!(
        j.cancel(&a.id, 1),
        Ok(TransitionOutcome::Durable(_))
    ));
    assert_eq!(j.sequence(), 3);
    drop(j);
    let mut reopened = open(&t, rel);
    assert_eq!(
        reopened.status(&a.id).unwrap().state,
        SubmissionState::Cancelled
    );
    assert_eq!(
        reopened.status(&b.id).unwrap().state,
        SubmissionState::Pending
    );
    assert_eq!(reopened.status(&a.id).unwrap().transition_revision, 2);
    assert!(matches!(
        enqueue(&mut reopened, a.clone(), 200),
        Ok(EnqueueOutcome::Idempotent(_))
    ));
    let mut conflicting = a.clone();
    conflicting.content = "different".into();
    conflicting.payload_hash =
        Submission::canonical_payload_hash(&conflicting.content, &conflicting.attachments).unwrap();
    assert!(matches!(
        enqueue(&mut reopened, conflicting, 200),
        Err(JournalError::DuplicatePayloadConflict)
    ));
}

#[test]
fn cancel_and_finalize_share_one_durable_revision_winner() {
    let t = Temp::new();
    let mut j = open(&t, Uuid::new_v4());
    let s = submission("c", "s", "payload");
    enqueue(&mut j, s.clone(), 10).unwrap();
    let captured = capture();
    assert!(matches!(
        j.claim_finalize(&s.id, 1, captured.clone()),
        Ok(TransitionOutcome::Durable(_))
    ));
    assert!(matches!(
        j.claim_finalize(&s.id, 1, captured),
        Ok(TransitionOutcome::Idempotent(_))
    ));
    assert!(matches!(
        j.cancel(&s.id, 1),
        Err(JournalError::InvalidState) | Err(JournalError::StaleRevision { .. })
    ));
    assert_eq!(j.status(&s.id).unwrap().state, SubmissionState::Finalizing);
    drop(j);
    let mut recovered = RelJournal::open(
        t.path(),
        RelOwnerId::parse_canonical(&format!("rel-{}", read_owner(&t))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        recovered.status(&s.id).unwrap().state,
        SubmissionState::Finalizing
    );
    assert!(matches!(
        recovered.cancel(&s.id, 2),
        Err(JournalError::InvalidState)
    ));
}
fn read_owner(t: &Temp) -> Uuid {
    let entries = fs::read_dir(t.path().join("conversation-queues")).unwrap();
    let path = entries
        .map(|x| x.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "rqj"))
        .unwrap();
    let bytes = fs::read(path).unwrap();
    Uuid::from_slice(&bytes[8..24]).unwrap()
}

#[test]
fn finalizing_replay_requires_exact_archive_identity_and_postcommit_receipt() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut j = open(&t, rel);
    let s = submission("c", "turn-1", "body");
    enqueue(&mut j, s.clone(), 44).unwrap();
    j.claim_finalize(&s.id, 1, capture()).unwrap();
    let good = receipt(&s, &capture());
    let mut bad = good.clone();
    bad.parent_node_id = Some("wrong-parent".into());
    assert!(matches!(
        j.reconcile_archive_node(&s.id, bad),
        Err(JournalError::ArchiveIdentityConflict)
    ));
    j.reconcile_archive_node(&s.id, good.clone()).unwrap();
    assert!(matches!(
        j.record_archive_commit(&s.id, good),
        Ok(TransitionOutcome::Idempotent(_))
    ));
    j.record_postcommit_receipt(
        &s.id,
        PostCommitReceipt {
            operation_id: "op-1".into(),
            scheduler_receipt: "sched-1".into(),
        },
    )
    .unwrap();
    drop(j);
    let recovered = open(&t, rel);
    let status = recovered.status(&s.id).unwrap();
    assert_eq!(status.state, SubmissionState::Committed);
    assert!(status.archive_receipt.is_some());
    assert!(status.post_commit_receipt.is_some());
}

#[test]
fn compaction_keeps_finalizing_and_tombstones_then_reopens_snapshot() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut j = open(&t, rel);
    let canceled = submission("a", "same", "cancelled-body");
    let finalizing = submission("b", "same", "finalizing-body");
    enqueue(&mut j, canceled.clone(), 50).unwrap();
    j.cancel(&canceled.id, 1).unwrap();
    enqueue(&mut j, finalizing.clone(), 50).unwrap();
    j.claim_finalize(&finalizing.id, 1, capture()).unwrap();
    j.compact().unwrap();
    drop(j);
    let recovered = open(&t, rel);
    assert_eq!(recovered.sequence(), 4);
    let c = recovered.status(&canceled.id).unwrap();
    assert_eq!(c.state, SubmissionState::Cancelled);
    assert!(!c.payload_retained);
    drop(recovered);
    let mut recovered = open(&t, rel);
    assert!(matches!(
        enqueue(&mut recovered, canceled.clone(), 900),
        Ok(EnqueueOutcome::Idempotent(_))
    ));
    let f = recovered.status(&finalizing.id).unwrap();
    assert_eq!(f.state, SubmissionState::Finalizing);
    assert!(f.payload_retained);
    assert_eq!(f.finalize_capture, Some(capture()));
}

#[test]
fn recovery_truncates_only_incomplete_final_frame_and_fails_closed_on_complete_corruption() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut j = open(&t, rel);
    let s = submission("c", "s", "payload");
    enqueue(&mut j, s.clone(), 1).unwrap();
    drop(j);
    let file = fs::read_dir(t.path().join("conversation-queues"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let good_len = fs::metadata(&file).unwrap().len();
    let mut f = OpenOptions::new().append(true).open(&file).unwrap();
    f.write_all(&[12, 0, 0]).unwrap();
    f.sync_all().unwrap();
    drop(f);
    let recovered = open(&t, rel);
    assert_eq!(recovered.sequence(), 1);
    assert_eq!(fs::metadata(&file).unwrap().len(), good_len);
    drop(recovered);
    let mut bytes = fs::read(&file).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x80;
    fs::write(&file, &bytes).unwrap();
    assert!(matches!(
        RelJournal::open(t.path(), RelOwnerId::from_uuid(rel)),
        Err(JournalError::Corrupt("complete frame checksum mismatch"))
    ));
}

#[test]
fn equal_timestamp_tie_order_is_stable_across_reopen() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut journal = open(&t, rel);
    for item_id in ["a", "b", "c", "d"] {
        let item = submission("ties", item_id, item_id);
        enqueue(&mut journal, item, 77).unwrap();
    }
    let order = journal.pending_order(&id("ties"));
    assert_eq!(order.len(), 4);
    drop(journal);
    let recovered = open(&t, rel);
    assert_eq!(recovered.pending_order(&id("ties")), order);
}

#[test]
fn staged_attachment_is_synced_and_required_before_durable_enqueue() {
    let t = Temp::new();
    let mut j = open(&t, Uuid::new_v4());
    let staged = j
        .stage_attachment(
            "file-1",
            "file.txt",
            Some("text/plain".into()),
            b"attachment bytes",
        )
        .unwrap();
    assert_eq!(j.read_attachment(&staged).unwrap(), b"attachment bytes");
    let mut s = submission("c", "with-file", "body");
    s.attachments = vec![staged];
    s.payload_hash = Submission::canonical_payload_hash(&s.content, &s.attachments).unwrap();
    assert!(matches!(
        enqueue(&mut j, s.clone(), 2),
        Ok(EnqueueOutcome::Durable(_))
    ));
    let unstaged = AttachmentIdentity {
        attachment_id: "not-staged".into(),
        filename: "missing.txt".into(),
        mime_type: None,
        content_hash: "00".repeat(32),
    };
    let mut bad = submission("c", "missing-file", "other");
    bad.attachments = vec![unstaged];
    bad.payload_hash = Submission::canonical_payload_hash(&bad.content, &bad.attachments).unwrap();
    assert!(matches!(
        enqueue(&mut j, bad, 3),
        Err(JournalError::MissingStagedAttachment)
    ));
}

#[test]
fn rel_scoped_gc_does_not_delete_another_rels_shared_hash() {
    let temp = Temp::new();
    let rel_a = Uuid::new_v4();
    let rel_b = Uuid::new_v4();
    let mut a = open(&temp, rel_a);
    let mut b = open(&temp, rel_b);
    let bytes = b"same bytes in two RELs";
    let identity_a = a.stage_attachment("a", "same.bin", None, bytes).unwrap();
    let identity_b = b.stage_attachment("b", "same.bin", None, bytes).unwrap();
    assert_eq!(identity_a.content_hash, identity_b.content_hash);

    let blob_a = temp
        .path()
        .join("conversation-queues")
        .join("staging")
        .join(rel_a.to_string())
        .join(format!("{}.blob", identity_a.content_hash));
    let blob_b = temp
        .path()
        .join("conversation-queues")
        .join("staging")
        .join(rel_b.to_string())
        .join(format!("{}.blob", identity_b.content_hash));
    assert!(blob_a.exists());
    assert!(blob_b.exists());

    a.compact().unwrap();
    assert!(!blob_a.exists());
    assert!(
        blob_b.exists(),
        "compacting one REL must not reclaim the same content hash staged by another REL"
    );
    b.compact().unwrap();
    assert!(!blob_b.exists());
}

#[test]
fn cancel_and_finalize_racing_threads_have_exactly_one_durable_winner() {
    let t = Temp::new();
    let mut journal = open(&t, Uuid::new_v4());
    let item = submission("race", "one", "payload");
    enqueue(&mut journal, item.clone(), 10).unwrap();
    let shared = Arc::new(Mutex::new(journal));
    let start = Arc::new(Barrier::new(3));
    let cancel_shared = Arc::clone(&shared);
    let cancel_start = Arc::clone(&start);
    let cancel_id = item.id.clone();
    let cancel = thread::spawn(move || {
        cancel_start.wait();
        cancel_shared.lock().unwrap().cancel(&cancel_id, 1)
    });
    let finalize_shared = Arc::clone(&shared);
    let finalize_start = Arc::clone(&start);
    let finalize_id = item.id.clone();
    let finalize = thread::spawn(move || {
        finalize_start.wait();
        finalize_shared
            .lock()
            .unwrap()
            .claim_finalize(&finalize_id, 1, capture())
    });
    start.wait();
    let cancel_result = cancel.join().unwrap();
    let finalize_result = finalize.join().unwrap();
    let durable = usize::from(matches!(cancel_result, Ok(TransitionOutcome::Durable(_))))
        + usize::from(matches!(finalize_result, Ok(TransitionOutcome::Durable(_))));
    assert_eq!(durable, 1);
    let state = shared.lock().unwrap().status(&item.id).unwrap().state;
    assert!(matches!(
        state,
        SubmissionState::Cancelled | SubmissionState::Finalizing
    ));
}

#[test]
fn full_rel_snapshot_may_exceed_single_frame_limit() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut journal = open(&t, rel);
    let body = "x".repeat(9 * 1024 * 1024);
    let a = submission("large-a", "one", &body);
    let b = submission("large-b", "two", &body);
    enqueue(&mut journal, a.clone(), 10).unwrap();
    enqueue(&mut journal, b.clone(), 11).unwrap();
    assert!(fs::metadata(journal.path()).unwrap().len() > FRAME_LIMIT as u64);
    journal.compact().unwrap();
    assert!(fs::metadata(journal.path()).unwrap().len() > FRAME_LIMIT as u64);
    drop(journal);
    let recovered = open(&t, rel);
    assert_eq!(recovered.sequence(), 2);
    assert_eq!(
        recovered.submission(&a.id).unwrap().content.len(),
        body.len()
    );
    assert_eq!(
        recovered.submission(&b.id).unwrap().content.len(),
        body.len()
    );
}

#[test]
fn native_cancel_and_finalize_require_terminal_host_outcomes() {
    let t = Temp::new();
    let mut journal = open(&t, Uuid::new_v4());
    let canceled = submission("host-gates", "cancel", "payload");
    enqueue(&mut journal, canceled.clone(), 10).unwrap();
    let capability = RuntimeCapability::ConversationExecution;
    let digest = [9u8; 32];
    journal
        .begin_host_handoff(&canceled.id, capability, "cancel-op".into(), digest)
        .unwrap();
    let receipt = HandoffReceipt {
        stable_id: "cancel-op".into(),
        payload_digest: digest,
        durably_accepted: true,
    };
    journal
        .record_host_handoff(&canceled.id, capability, &receipt)
        .unwrap();
    let revision = journal.conversation_revision(&canceled.id.conversation_id);
    assert!(matches!(
        journal.cancel(&canceled.id, revision),
        Err(JournalError::InvalidState)
    ));
    assert!(matches!(
        journal.claim_finalize(&canceled.id, revision, capture()),
        Err(JournalError::InvalidState)
    ));
    journal
        .record_host_status(&canceled.id, capability, HostOperationStatus::Unknown)
        .unwrap();
    assert!(matches!(
        journal.cancel(&canceled.id, revision),
        Err(JournalError::InvalidState)
    ));
    journal
        .record_host_status(&canceled.id, capability, HostOperationStatus::Cancelled)
        .unwrap();
    journal.cancel(&canceled.id, revision).unwrap();
    assert_eq!(
        journal.status(&canceled.id).unwrap().state,
        SubmissionState::Cancelled
    );

    let completed = submission("host-gates", "complete", "payload");
    enqueue(&mut journal, completed.clone(), 11).unwrap();
    journal
        .begin_host_handoff(&completed.id, capability, "complete-op".into(), digest)
        .unwrap();
    let receipt = HandoffReceipt {
        stable_id: "complete-op".into(),
        payload_digest: digest,
        durably_accepted: true,
    };
    journal
        .record_host_handoff(&completed.id, capability, &receipt)
        .unwrap();
    let revision = journal.conversation_revision(&completed.id.conversation_id);
    assert!(matches!(
        journal.claim_finalize(&completed.id, revision, capture()),
        Err(JournalError::InvalidState)
    ));
    journal
        .record_host_status(&completed.id, capability, HostOperationStatus::Completed)
        .unwrap();
    journal
        .claim_finalize(&completed.id, revision, capture())
        .unwrap();
    assert_eq!(
        journal.status(&completed.id).unwrap().state,
        SubmissionState::Finalizing
    );
}

#[test]
fn compact_host_payload_frames_attachment_bytes_without_json_expansion() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut journal = open(&t, rel);
    let attachment = journal
        .stage_attachment(
            "blob-1",
            "payload.bin",
            Some("application/octet-stream".into()),
            &[0, 1, 255],
        )
        .unwrap();
    let mut item = submission("host-frame", "one", "message");
    item.attachments = vec![attachment];
    item.payload_hash =
        Submission::canonical_payload_hash(&item.content, &item.attachments).unwrap();
    enqueue(&mut journal, item.clone(), 10).unwrap();

    let (payload, digest) = journal.host_payload("rel-test", &item.id).unwrap();
    assert_eq!(&payload[..8], b"RQMUXH01");
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&payload)), digest);
    let metadata_len = u64::from_le_bytes(payload[8..16].try_into().unwrap()) as usize;
    let attachment_header = 16 + metadata_len;
    let attachment_len = u64::from_le_bytes(
        payload[attachment_header..attachment_header + 8]
            .try_into()
            .unwrap(),
    );
    assert_eq!(attachment_len, 3);
    assert_eq!(&payload[attachment_header + 8..], &[0, 1, 255]);
}

#[test]
fn rel_file_has_one_lifetime_writer_lock() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let first = open(&t, rel);
    assert!(matches!(
        RelJournal::open(t.path(), RelOwnerId::from_uuid(rel)),
        Err(JournalError::Io(_))
    ));
    drop(first);
    assert!(RelJournal::open(t.path(), RelOwnerId::from_uuid(rel)).is_ok());
}

#[test]
fn host_cancel_intent_is_sticky_through_unknown_compaction_and_reopen() {
    let t = Temp::new();
    let rel = Uuid::new_v4();
    let mut journal = open(&t, rel);
    let item = submission("host-cancel", "one", "payload");
    enqueue(&mut journal, item.clone(), 10).unwrap();
    let digest = [7u8; 32];
    journal
        .begin_host_handoff(
            &item.id,
            RuntimeCapability::ConversationExecution,
            "stable-host-op-1".into(),
            digest,
        )
        .unwrap();
    journal
        .begin_host_cancel(&item.id, RuntimeCapability::ConversationExecution)
        .unwrap();
    let late_receipt = HandoffReceipt {
        stable_id: "stable-host-op-1".into(),
        payload_digest: digest,
        durably_accepted: true,
    };
    journal
        .record_host_handoff(
            &item.id,
            RuntimeCapability::ConversationExecution,
            &late_receipt,
        )
        .unwrap();
    assert_eq!(
        journal
            .host_operation(&item.id, RuntimeCapability::ConversationExecution)
            .unwrap()
            .phase,
        DurableHostOperationPhase::CancelIntent
    );
    journal
        .record_host_status(
            &item.id,
            RuntimeCapability::ConversationExecution,
            HostOperationStatus::Unknown,
        )
        .unwrap();
    let operation = journal
        .host_operation(&item.id, RuntimeCapability::ConversationExecution)
        .unwrap();
    assert!(operation.cancel_requested);
    assert_eq!(operation.phase, DurableHostOperationPhase::Unknown);
    assert_eq!(
        operation.last_known_phase,
        Some(DurableHostOperationPhase::CancelIntent)
    );
    journal.compact().unwrap();
    drop(journal);

    let recovered = open(&t, rel);
    let status = recovered.status(&item.id).unwrap();
    assert_eq!(status.state, SubmissionState::Pending);
    assert!(status.payload_retained);
    let operation = status
        .host_operations
        .get(&RuntimeCapability::ConversationExecution)
        .unwrap();
    assert!(operation.cancel_requested);
    assert_eq!(operation.phase, DurableHostOperationPhase::Unknown);
    assert_eq!(
        operation.last_known_phase,
        Some(DurableHostOperationPhase::CancelIntent)
    );
}

#[cfg(windows)]
#[test]
fn long_windows_journal_path_compacts_and_reopens() {
    let base = std::env::temp_dir().join(format!("reliquary-long-{}", Uuid::new_v4()));
    let mut long_root = base.clone();
    for index in 0..5 {
        long_root = long_root.join(format!("segment-{index}-{}", "x".repeat(45)));
    }
    fs::create_dir_all(&long_root).unwrap();
    let temp = Temp(base);
    let rel = Uuid::new_v4();
    let mut journal = RelJournal::open(temp.path(), RelOwnerId::from_uuid(rel)).unwrap();
    let item = submission("long-path", "one", "payload");
    enqueue(&mut journal, item.clone(), 10).unwrap();
    journal.compact().unwrap();
    drop(journal);
    let recovered = RelJournal::open(temp.path(), RelOwnerId::from_uuid(rel)).unwrap();
    assert_eq!(recovered.sequence(), 1);
    assert_eq!(recovered.submission(&item.id).unwrap().content, "payload");
}
