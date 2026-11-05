use super::*;
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_vector_store::{
    ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
    VectorGenerationStore,
};

fn initialize_rel(path: &Path, owner: [u8; 16]) {
    let mut container = Container::create(
        path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    container.sync().unwrap();
}

#[test]
fn writable_mount_queue_finalization_and_reopen_retry_are_end_to_end() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-finalize-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let queue_path = root.join("queue");
    let raw_owner = [61; 16];
    initialize_rel(&rel_path, raw_owner);

    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &admission, [], [])
        .unwrap();
    let instance = gateway.instance("alice", [owner], []).unwrap();
    let id =
        ConversationSubmissionId::new("conversation-1", SubmissionId::new("submission-1").unwrap())
            .unwrap();
    let capture = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: None,
        committed_timestamp_ns: 42,
        verified_principal_id: "alice".to_owned(),
    };

    let mut queue = instance
        .open_conversation_queue(&queue_path, owner)
        .unwrap();
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            id.submission_id.clone(),
            "durable conversation text",
            0,
        )
        .unwrap();
    let other_principal = gateway.instance("bob", [owner], []).unwrap();
    assert!(matches!(
        other_principal.finalize_conversation_submission(&mut queue, &id, 1, capture.clone(),),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    assert_eq!(
        instance
            .conversation_submission_status(&queue, &id)
            .unwrap()
            .unwrap()
            .state,
        QueueState::Pending
    );
    let committed = instance
        .finalize_conversation_submission(&mut queue, &id, 1, capture.clone())
        .unwrap();
    assert_eq!(committed.state, QueueState::Committed);
    assert_eq!(
        committed.archive_receipt.as_ref().unwrap().principal_id,
        "alice"
    );
    assert!(matches!(
        other_principal.finalize_conversation_submission(&mut queue, &id, 1, capture.clone(),),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    assert!(
        instance
            .with_writable_reliquary(owner, |rel| {
                rel.archive_node("conversation-1", "submission-1").is_some()
            })
            .unwrap()
    );

    let followup_id =
        ConversationSubmissionId::new("conversation-1", SubmissionId::new("submission-2").unwrap())
            .unwrap();
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            followup_id.submission_id.clone(),
            "forked follow-up",
            3,
        )
        .unwrap();
    let stale_parent = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: None,
        committed_timestamp_ns: 43,
        verified_principal_id: "alice".to_owned(),
    };
    assert!(matches!(
        instance.finalize_conversation_submission(&mut queue, &followup_id, 4, stale_parent,),
        Err(GatewayError::OwnerSession(_))
    ));
    assert_eq!(
        instance
            .conversation_submission_status(&queue, &followup_id)
            .unwrap()
            .unwrap()
            .state,
        QueueState::Pending
    );
    assert!(
        !instance
            .with_writable_reliquary(owner, |rel| {
                rel.archive_node("conversation-1", "submission-2").is_some()
            })
            .unwrap()
    );
    let fork_capture = FinalizeCapture {
        branch_id: "fork".to_owned(),
        parent_node_id: Some("submission-1".to_owned()),
        committed_timestamp_ns: 43,
        verified_principal_id: "alice".to_owned(),
    };
    let forked = instance
        .finalize_conversation_submission(&mut queue, &followup_id, 4, fork_capture.clone())
        .unwrap();
    assert_eq!(forked.state, QueueState::Committed);

    let outsider = gateway.instance("bob", [], []).unwrap();
    assert!(matches!(
        outsider.open_conversation_queue(&queue_path, owner),
        Err(GatewayError::UnauthorizedRel { .. })
    ));

    drop(queue);
    let mut recovered = instance
        .open_conversation_queue(&queue_path, owner)
        .unwrap();
    assert_eq!(
        instance
            .finalize_conversation_submission(&mut recovered, &followup_id, 4, fork_capture)
            .unwrap(),
        forked
    );
    let (accepted_turn_count, content) = instance
        .with_writable_reliquary(owner, |rel| {
            (
                rel.accepted_turn_sources().len(),
                rel.resolved_archive_turn("conversation-1", "submission-2")
                    .unwrap()
                    .unwrap()
                    .content,
            )
        })
        .unwrap();
    assert_eq!(accepted_turn_count, 2);
    assert_eq!(content, "forked follow-up");
    drop(recovered);
    drop(instance);
    drop(gateway);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn archive_commit_without_queue_receipt_reconciles_after_branch_advances() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-archive-reconcile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let queue_path = root.join("queue");
    let raw_owner = [62; 16];
    initialize_rel(&rel_path, raw_owner);

    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &admission, [], [])
        .unwrap();
    let instance = gateway.instance("alice", [owner], []).unwrap();
    let first_id =
        ConversationSubmissionId::new("conversation-1", SubmissionId::new("submission-1").unwrap())
            .unwrap();
    let second_id =
        ConversationSubmissionId::new("conversation-1", SubmissionId::new("submission-2").unwrap())
            .unwrap();
    let first_capture = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: None,
        committed_timestamp_ns: 43,
        verified_principal_id: "alice".to_owned(),
    };
    let second_capture = FinalizeCapture {
        branch_id: "main".to_owned(),
        parent_node_id: Some("submission-1".to_owned()),
        committed_timestamp_ns: 44,
        verified_principal_id: "alice".to_owned(),
    };
    let mut queue = instance
        .open_conversation_queue(&queue_path, owner)
        .unwrap();
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            first_id.submission_id.clone(),
            "first turn",
            0,
        )
        .unwrap();
    queue
        .claim_finalize(&first_id, 1, first_capture.clone())
        .unwrap();

    // Simulate process loss after both Archive frames sync but before the
    // queue's ArchiveCommitted event is appended.
    {
        let owners = instance.owners.read().unwrap();
        let Some(MountedOwner::RelWriter(mounted)) = owners.get(&owner) else {
            panic!("writable REL mount disappeared");
        };
        mounted
            .writer
            .lock()
            .unwrap()
            .accept_turn_and_advance_branch(
                reliquary_archive::TurnInput {
                    id: "submission-1".to_owned(),
                    conversation_id: "conversation-1".to_owned(),
                    parent_id: None,
                    role: "user".to_owned(),
                    principal_id: Some("alice".to_owned()),
                    timestamp_ns: 43,
                    content: "first turn".to_owned(),
                    attachments: Vec::new(),
                    project_attachments: Vec::new(),
                },
                reliquary_archive::Branch {
                    id: "main".to_owned(),
                    conversation_id: "conversation-1".to_owned(),
                    leaf_node_id: "submission-1".to_owned(),
                    canonical: true,
                },
            )
            .unwrap();
    }
    assert!(queue.status(&first_id).unwrap().archive_receipt.is_none());
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            second_id.submission_id.clone(),
            "second turn",
            2,
        )
        .unwrap();
    instance
        .finalize_conversation_submission(&mut queue, &second_id, 3, second_capture)
        .unwrap();
    drop(queue);

    let mut recovered = instance
        .open_conversation_queue(&queue_path, owner)
        .unwrap();
    let first_status = instance
        .finalize_conversation_submission(&mut recovered, &first_id, 1, first_capture)
        .unwrap();
    assert_eq!(first_status.state, QueueState::Committed);
    assert!(first_status.archive_receipt.is_some());
    let accepted_count = instance
        .with_writable_reliquary(owner, |rel| rel.accepted_turn_sources().len())
        .unwrap();
    assert_eq!(accepted_count, 2);

    drop(recovered);
    drop(instance);
    drop(gateway);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ungranted_owner_access_is_denied_before_lookup() {
    let gateway = Gateway::new();
    let owner = OwnerId::from_bytes([41; 16]);
    let instance = gateway.instance("alice", [], []).unwrap();
    assert!(matches!(
        instance.reliquary(owner),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    assert!(matches!(
        instance.phylactery(owner),
        Err(GatewayError::UnauthorizedPhy { .. })
    ));
}

#[test]
fn grants_cannot_name_unmounted_owners() {
    let gateway = Gateway::new();
    let rel = OwnerId::from_bytes([42; 16]);
    let phy = OwnerId::from_bytes([43; 16]);
    assert!(
        matches!(gateway.instance("alice", [rel], []), Err(GatewayError::UnknownOwner(id)) if id == rel)
    );
    assert!(
        matches!(gateway.instance("alice", [], [phy]), Err(GatewayError::UnknownOwner(id)) if id == phy)
    );
}

#[test]
fn duplicate_owner_mount_is_rejected_without_replacing_the_canonical_session() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-duplicate-mount-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let raw_owner = [44; 16];
    initialize_rel(&rel_path, raw_owner);

    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
    let owner = gateway
        .mount_reliquary(&rel_path, &admission, [], [])
        .unwrap();
    assert!(matches!(
        gateway.mount_reliquary(&rel_path, &admission, [], []),
        Err(GatewayError::DuplicateOwner(duplicate)) if duplicate == owner
    ));
    assert_eq!(gateway.mounted_owner_count().unwrap(), 1);
    assert_eq!(
        gateway
            .instance("alice", [owner], [])
            .unwrap()
            .reliquary(owner)
            .unwrap()
            .owner_id(),
        raw_owner
    );

    drop(gateway);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn writable_mount_excludes_a_second_mutable_owner_session() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-exclusive-writer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let raw_owner = [45; 16];
    initialize_rel(&rel_path, raw_owner);

    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &admission, [], [])
        .unwrap();
    let instance = gateway.instance("alice", [owner], []).unwrap();

    assert!(matches!(
        RelWriter::open_path(&rel_path, admission, [], []),
        Err(OwnerSessionError::Container(
            reliquary_container::ContainerError::WriterConflict
        ))
    ));

    instance
        .with_rel_writer(owner, |writer| {
            writer.accept_turn_and_advance_branch(
                reliquary_archive::TurnInput {
                    id: "canonical-turn".to_owned(),
                    conversation_id: "conversation".to_owned(),
                    parent_id: None,
                    role: "user".to_owned(),
                    principal_id: Some("alice".to_owned()),
                    timestamp_ns: 1,
                    content: "canonical writer".to_owned(),
                    attachments: Vec::new(),
                    project_attachments: Vec::new(),
                },
                reliquary_archive::Branch {
                    id: "main".to_owned(),
                    conversation_id: "conversation".to_owned(),
                    leaf_node_id: "canonical-turn".to_owned(),
                    canonical: true,
                },
            )
        })
        .unwrap()
        .unwrap();
    drop((instance, gateway));

    let reopened = RelSession::open_path(
        &rel_path,
        &GraphAdmission::for_replay(raw_owner, [], [], [], [], []),
        [],
        [],
    )
    .unwrap();
    assert!(
        reopened
            .archive_node("conversation", "canonical-turn")
            .is_some()
    );

    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn blank_or_control_character_principals_are_rejected() {
    let gateway = Gateway::new();
    assert!(matches!(
        gateway.instance("  ", [], []),
        Err(GatewayError::InvalidPrincipal)
    ));
    assert!(matches!(
        gateway.instance("alice\nadmin", [], []),
        Err(GatewayError::InvalidPrincipal)
    ));
}

#[test]
fn host_admission_binds_bearer_credential_to_configured_principal() {
    let gateway = Gateway::new();
    let token = "correct horse battery staple with enough entropy";
    let host = TrustedHostAdmission::new([HostCredentialBinding::new("alice", token, [], [])]);
    let (adapter, _control) = host.install(&gateway).unwrap();

    assert_eq!(adapter.authenticate(token).unwrap().principal(), "alice");
    assert!(matches!(
        adapter.authenticate("attacker supplied a different sufficiently long token"),
        Err(GatewayError::InvalidCredential)
    ));
}

#[test]
fn host_admission_rejects_bad_credentials_and_duplicate_tokens() {
    let gateway = Gateway::new();
    assert!(matches!(
        TrustedHostAdmission::new([HostCredentialBinding::new("alice", "short", [], [])])
            .install(&gateway),
        Err(GatewayError::InvalidCredential)
    ));
    let token = "shared credential with at least thirty two bytes";
    assert!(matches!(
        TrustedHostAdmission::new([
            HostCredentialBinding::new("alice", token, [], []),
            HostCredentialBinding::new("bob", token, [], []),
        ])
        .install(&gateway),
        Err(GatewayError::DuplicateCredential)
    ));
}

#[test]
fn host_revocation_invalidates_existing_instances_and_future_admission() {
    let gateway = Gateway::new();
    let token = "revocable host credential with enough bytes";
    let host = TrustedHostAdmission::new([HostCredentialBinding::new("alice", token, [], [])]);
    let (adapter, control) = host.install(&gateway).unwrap();
    let instance = adapter.authenticate(token).unwrap();

    control.revoke(token).unwrap();
    assert!(matches!(
        instance.reliquary(OwnerId::from_bytes([41; 16])),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        instance.claim_next_insomnia_input(OwnerId::from_bytes([41; 16]), "worker", 1, 10, 1),
        Err(GatewayError::CredentialRevoked)
    ));
    assert!(matches!(
        adapter.authenticate(token),
        Err(GatewayError::CredentialRevoked)
    ));
}

#[test]
fn ungranted_instance_cannot_submit_to_an_owner_queue() {
    let dir = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-denied-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    let gateway = Gateway::new();
    let owner = OwnerId::from_bytes([23; 16]);
    let instance = gateway.instance("alice", [], []).unwrap();
    let mut queue = ConversationQueue::open(&dir, owner).unwrap();
    let result = instance.submit_conversation(
        &mut queue,
        "conversation",
        SubmissionId::new("submission").unwrap(),
        "content",
        0,
    );
    assert!(matches!(result, Err(GatewayError::UnauthorizedRel { .. })));
    assert_eq!(queue.sequence(), 0);
    drop(queue);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn only_the_submitting_principal_can_cancel_and_the_winner_survives_reopen() {
    let root = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-cancel-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let rel_path = root.join("owner.rel");
    let queue_path = root.join("queue");
    let raw_owner = [63; 16];
    initialize_rel(&rel_path, raw_owner);

    let gateway = Gateway::new();
    let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
    let owner = gateway
        .mount_writable_reliquary(&rel_path, &admission, [], [])
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let bob = gateway.instance("bob", [owner], []).unwrap();
    let id = ConversationSubmissionId::new(
        "conversation",
        SubmissionId::new("cancelled-submission").unwrap(),
    )
    .unwrap();
    let mut queue = alice.open_conversation_queue(&queue_path, owner).unwrap();
    alice
        .submit_conversation(
            &mut queue,
            id.conversation_id.clone(),
            id.submission_id.clone(),
            "cancel this submission",
            0,
        )
        .unwrap();

    assert!(matches!(
        bob.cancel_own_pending_submission(&mut queue, &id, 1),
        Err(GatewayError::UnauthorizedQueueSubmission { .. })
    ));
    let cancelled = alice
        .cancel_own_pending_submission(&mut queue, &id, 1)
        .unwrap();
    assert_eq!(cancelled.state, QueueState::Cancelled);
    assert_eq!(
        alice
            .cancel_own_pending_submission(&mut queue, &id, 0)
            .unwrap(),
        cancelled
    );
    drop(queue);

    let reopened = alice.open_conversation_queue(&queue_path, owner).unwrap();
    assert_eq!(
        alice
            .conversation_submission_status(&reopened, &id)
            .unwrap(),
        Some(cancelled)
    );

    drop((reopened, alice, bob, gateway));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ungranted_instance_cannot_claim_insomnia_work() {
    let gateway = Gateway::new();
    let owner = OwnerId::from_bytes([88; 16]);
    let instance = gateway.instance("alice", [], []).unwrap();
    assert!(matches!(
        instance.claim_next_insomnia_input(owner, "worker", 1, 10, 1),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
}

#[test]
fn ungranted_instance_cannot_read_generation_checkpoint_or_replay() {
    let dir = std::env::current_dir()
        .unwrap()
        .join(".test-tmp")
        .join(format!(
            "gateway-stream-denied-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    let gateway = Gateway::new();
    let owner = OwnerId::from_bytes([28; 16]);
    let instance = gateway.instance("alice", [], []).unwrap();
    let queue = ConversationQueue::open(&dir, owner).unwrap();
    let id =
        ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
            .unwrap();
    assert!(matches!(
        instance.generation_checkpoint_snapshot(&queue, &id),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    assert!(matches!(
        instance.replay_generation_checkpoints(&queue, &id, 0, 1),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    drop(queue);
    std::fs::remove_dir_all(dir).unwrap();
}
