use super::*;

const CONVERSATION: &str = "conversation-one";

fn writer(g: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    g.attach_instance(
        "recovery-principal".into(),
        None,
        grants(
            owner,
            &[
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::ManageRel,
            ],
        ),
    )
    .unwrap()
}

fn submit(
    g: &GatewayRuntime,
    w: &InstanceRuntime,
    owner: &str,
    id: &str,
    branch: &str,
    parent: &str,
) -> GatewaySubmissionReceipt {
    let mut request = submission(CONVERSATION, id, parent);
    request.branch_id = branch.into();
    request.expected_conversation_revision = g
        .conversation
        .journal(owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new(CONVERSATION).unwrap());
    g.submit_conversation(w, owner, request).unwrap()
}

fn start(g: &GatewayRuntime, w: &InstanceRuntime, owner: &str) -> GatewaySubmissionReceipt {
    submit(g, w, owner, "generation-owner", "main", "root-one");
    g.process_next_accepted_conversation_submission(owner, CONVERSATION)
        .unwrap()
        .unwrap();
    g.start_conversation_generation(owner, CONVERSATION, "generation-owner")
        .unwrap()
}

#[test]
fn coordinator_exact_acquisition_requires_existing_work_and_its_durable_branch() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let pending = submit(&g, &w, &owner, "fresh", "main", "root-one");
    let journal = g.conversation.journal(&owner).unwrap();
    let coordinator = g.conversation.coordinator(&owner, CONVERSATION).unwrap();
    let before = journal.lock().unwrap().sequence();
    assert!(coordinator.acquire(&journal, &pending.id, "main").is_err());
    assert!(
        g.resume_conversation_submission(&w, &owner, CONVERSATION, "fresh")
            .is_err()
    );
    assert_eq!(journal.lock().unwrap().sequence(), before);
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert!(
        coordinator
            .acquire(&journal, &pending.id, "alternate")
            .is_err()
    );
    let generation = g
        .start_conversation_generation(&owner, CONVERSATION, "fresh")
        .unwrap()
        .status
        .generation
        .unwrap();
    assert!(
        coordinator
            .acquire(&journal, &pending.id, "alternate")
            .is_err()
    );
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "reply",
    )
    .unwrap();
    g.complete_conversation_generation(&owner, CONVERSATION, &generation.operation_id)
        .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn repeated_mount_recovers_policy_queue_generation_and_no_viewer_lifetime() {
    let state = test_path("p38-mixed-state");
    let path = test_path("p38-mixed.rel");
    let g = gateway_at(state.clone());
    let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let w = writer(&g, &owner);
    g.set_conversation_policy(
        &w,
        &owner,
        CONVERSATION,
        crate::ConversationPolicy::VisibleAutoBranch,
    )
    .unwrap();
    let started = start(&g, &w, &owner);
    let generation = started.status.generation.unwrap();
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "saved prefix",
    )
    .unwrap();
    submit(&g, &w, &owner, "main-next", "main", "generation-owner");
    submit(&g, &w, &owner, "alt-next", "alternate", "root-one-alt");
    let viewer = g
        .attach_conversation_viewer(&w, &owner, CONVERSATION)
        .unwrap();
    let journal = g.conversation.journal(&owner).unwrap();
    let pending = journal
        .lock()
        .unwrap()
        .pending_order(&started.id.conversation_id);
    let before_version = {
        let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel(&token, |r| {
            r.cva
                .set_conversation_active(CONVERSATION, true)
                .map_err(operation)?;
            r.cva.sync().map_err(operation)?;
            Ok(r.cva.archive_version())
        })
        .unwrap()
    };
    drop(journal);
    // Crash with a live viewer and durable conversation_active metadata.
    drop(w);
    drop(g);
    let mut stable = None;
    for _ in 0..2 {
        let g = gateway_at(state.clone());
        assert_eq!(
            g.mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
                .unwrap(),
            owner
        );
        assert!(g.instances.read().unwrap().is_empty());
        assert!(g.conversation.viewers.lock().unwrap().viewers.is_empty());
        assert!(g.detach_conversation_viewer(viewer.clone()).is_err());
        let journal = g.conversation.journal(&owner).unwrap();
        assert_eq!(
            journal
                .lock()
                .unwrap()
                .pending_order(&started.id.conversation_id),
            pending
        );
        let recovered = journal
            .lock()
            .unwrap()
            .status(&started.id)
            .unwrap()
            .generation
            .unwrap();
        assert_eq!(recovered.operation_id, generation.operation_id);
        assert_eq!(recovered.generation_id, generation.generation_id);
        assert_eq!(recovered.phase, DurableGenerationPhase::Interrupted);
        assert_eq!(recovered.checkpoint.as_ref().unwrap().sequence, 1);
        let observer = writer(&g, &owner);
        let snapshot = g
            .conversation_observation_snapshot(&observer, &owner, CONVERSATION)
            .unwrap();
        assert_eq!(
            snapshot.policy,
            crate::ConversationPolicy::VisibleAutoBranch
        );
        assert_eq!(snapshot.policy_revision, 2);
        assert!(snapshot.selected_branch_id.is_none());
        if let Some(previous) = stable.as_ref() {
            assert_eq!(&snapshot, previous);
        } else {
            stable = Some(snapshot);
        }
        let token = g.authorize(&observer, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel(&token, |r| {
            assert_eq!(r.cva.archive_version(), before_version);
            assert!(
                r.sessions.is_empty(),
                "interruption and viewer recovery create no writable session"
            );
            Ok(())
        })
        .unwrap();
        drop(journal);
        g.detach_instance(&observer).unwrap();
        drop(g.unmount_rel(&owner).unwrap());
    }
    let g = gateway_at(state);
    g.mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
        .unwrap();
    for expected in pending {
        assert_eq!(
            g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
                .unwrap()
                .unwrap()
                .id,
            expected
        );
    }
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn mount_rejects_checkpoint_prefix_mismatch_instead_of_replacing_acknowledged_content() {
    for (length, claimed) in [
        (8, "prefix x"),
        (30, "prefix missing stream bytes"),
        (7, "otherxx"),
    ] {
        let state = test_path("p38-invalid-checkpoint-state");
        let path = test_path("p38-invalid-checkpoint.rel");
        let g = gateway_at(state.clone());
        let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
        let w = writer(&g, &owner);
        let started = start(&g, &w, &owner);
        let generation = started.status.generation.unwrap();
        g.checkpoint_conversation_generation(
            &owner,
            CONVERSATION,
            &generation.operation_id,
            1,
            "prefix",
        )
        .unwrap();
        let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel(&token, |r| {
            r.cva
                .put_interaction_stream(InteractionStreamRecord {
                    message_id: generation.generation_id.clone(),
                    session_id: CONVERSATION.into(),
                    parent_message_id: Some(generation.parent_node_id.clone()),
                    role: InteractionRole::Agent,
                    timestamp_ns: generation.started_timestamp_ns,
                    content: "prefix é tail".into(),
                    status: InteractionStreamStatus::Streaming,
                })
                .map_err(operation)?;
            r.cva.sync().map_err(operation)
        })
        .unwrap();
        let hash = Sha256::digest(claimed.as_bytes()).into();
        g.conversation
            .journal(&owner)
            .unwrap()
            .lock()
            .unwrap()
            .record_generation_checkpoint(&started.id, 2, length, hash)
            .unwrap();
        g.detach_instance(&w).unwrap();
        drop(g.unmount_rel(&owner).unwrap());
        drop(g);
        let recovered = gateway_at(state.clone());
        assert!(
            recovered
                .mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
                .is_err()
        );
        assert!(recovered.rel_executions.read().unwrap().is_empty());
        let journal =
            RelJournal::open(&state, RelOwnerId::parse_canonical(&owner).unwrap()).unwrap();
        let retained = journal.status(&started.id).unwrap().generation.unwrap();
        assert_eq!(retained.phase, DurableGenerationPhase::Running);
        assert_eq!(retained.checkpoint.unwrap().sequence, 2);
    }
}

#[test]
fn generation_archive_recovery_reconciles_stream_ahead_of_journal_before_completion() {
    let state = test_path("p38-completion-handoff-state");
    let path = test_path("p38-completion-handoff.rel");
    let g = gateway_at(state.clone());
    let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let w = writer(&g, &owner);
    let started = start(&g, &w, &owner);
    let generation = started.status.generation.unwrap();
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "prefix",
    )
    .unwrap();
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    g.with_rel(&token, |r| {
        r.cva
            .put_interaction_stream(InteractionStreamRecord {
                message_id: generation.generation_id.clone(),
                session_id: CONVERSATION.into(),
                parent_message_id: Some(generation.parent_node_id.clone()),
                role: InteractionRole::Agent,
                timestamp_ns: generation.started_timestamp_ns,
                content: "prefix extended".into(),
                status: InteractionStreamStatus::Streaming,
            })
            .map_err(operation)?;
        r.cva
            .append_node_with_principal(
                generation.generation_id.clone(),
                CONVERSATION.into(),
                Some(generation.parent_node_id.clone()),
                "assistant".into(),
                Some("recovery-principal".into()),
                generation.started_timestamp_ns,
                "prefix extended",
            )
            .map_err(operation)?;
        r.cva.sync().map_err(operation)
    })
    .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g);
    let g = gateway_at(state);
    g.mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
        .unwrap();
    let journal = g.conversation.journal(&owner).unwrap();
    let recovered = journal
        .lock()
        .unwrap()
        .status(&started.id)
        .unwrap()
        .generation
        .unwrap();
    assert_eq!(recovered.phase, DurableGenerationPhase::Completed);
    let checkpoint = recovered.checkpoint.unwrap();
    assert_eq!(checkpoint.sequence, 2);
    assert_eq!(checkpoint.utf8_bytes, "prefix extended".len() as u64);
    assert_eq!(
        checkpoint.sha256,
        <[u8; 32]>::from(Sha256::digest(b"prefix extended"))
    );
    drop(journal);
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn quiescence_rejects_mismatched_archive_turn_before_recording_postcommit() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let receipt = submit(&g, &w, &owner, "missing-publication", "main", "root-one");
    let journal = g.conversation.journal(&owner).unwrap();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        branch_label: None,
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: 123,
        verified_principal_id: "recovery-principal".into(),
        policy_revision: 1,
    };
    {
        let mut journal = journal.lock().unwrap();
        let payload = journal.submission(&receipt.id).unwrap().clone();
        let revision = journal.conversation_revision(&receipt.id.conversation_id);
        journal
            .claim_finalize(&receipt.id, revision, capture.clone())
            .unwrap();
        journal
            .record_archive_commit(
                &receipt.id,
                ArchiveReceipt {
                    node_id: receipt.id.submission_id.as_str().into(),
                    branch_id: capture.branch_id.clone(),
                    parent_node_id: capture.parent_node_id.clone(),
                    principal_id: capture.verified_principal_id.clone(),
                    committed_timestamp_ns: capture.committed_timestamp_ns,
                    payload_hash: payload.payload_hash,
                    attachment_identities: payload.attachments,
                },
            )
            .unwrap();
    }
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    g.with_rel(&token, |runtime| {
        runtime
            .cva
            .append_node_with_principal(
                receipt.id.submission_id.as_str().into(),
                CONVERSATION.into(),
                Some("root-one".into()),
                "user".into(),
                Some("recovery-principal".into()),
                capture.committed_timestamp_ns,
                "wrong durable content",
            )
            .map_err(operation)?;
        runtime
            .cva
            .append_branch(Branch {
                id: "main".into(),
                conversation_id: CONVERSATION.into(),
                leaf_node_id: receipt.id.submission_id.as_str().into(),
                canonical: true,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)
    })
    .unwrap();
    g.detach_instance(&w).unwrap();
    g.quiesce_rel(&owner).unwrap();
    assert!(g.reopen_rel(&owner).is_err());
    let status = journal.lock().unwrap().status(&receipt.id).unwrap();
    assert_eq!(status.state, SubmissionState::Committed);
    assert!(status.post_commit_receipt.is_none());
    assert_eq!(
        g.rel_executions.read().unwrap()[&owner]
            .fence
            .snapshot()
            .unwrap()
            .0,
        OwnerAdmission::Closed
    );
    assert!(
        g.rel_executions.read().unwrap()[&owner]
            .owner
            .lock()
            .unwrap()
            .workers_stopped()
    );
    drop(journal);
    drop(g.unmount_rel(&owner).unwrap());
}
