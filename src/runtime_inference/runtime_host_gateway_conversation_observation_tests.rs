use super::super::super::observation::{ConversationObservationSnapshot, GenerationStatus};
use super::*;
const CONVERSATION: &str = "conversation-one";

fn writer(g: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    g.attach_instance(
        "snapshot-private-principal".into(),
        None,
        grants(
            owner,
            &[
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::ManageRel,
                OwnerGrant::CancelOwnPendingSubmission,
                OwnerGrant::CancelSharedGeneration,
            ],
        ),
    )
    .unwrap()
}
fn snapshot(
    g: &GatewayRuntime,
    viewer: &InstanceRuntime,
    owner: &str,
) -> ConversationObservationSnapshot {
    g.conversation_observation_snapshot(viewer, owner, CONVERSATION)
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
    let journal = g.conversation.journal(owner).unwrap();
    let revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new(CONVERSATION).unwrap());
    let mut request = submission(CONVERSATION, id, parent);
    request.branch_id = branch.into();
    request.expected_conversation_revision = revision;
    request.content = "snapshot-private-body".into();
    g.submit_conversation(w, owner, request).unwrap()
}

#[test]
fn snapshot_is_scoped_ordered_private_and_preserves_compacted_terminal_entries() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let committed = submit(&g, &w, &owner, "committed", "main", "root-one");
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let cancelled = submit(&g, &w, &owner, "cancelled", "alternate", "root-one-alt");
    g.cancel_conversation_submission(&w, &owner, CONVERSATION, "cancelled")
        .unwrap();
    let pending = submit(&g, &w, &owner, "pending", "main", "committed");
    g.submit_conversation(
        &w,
        &owner,
        submission("conversation-two", "other-conversation", "root-two"),
    )
    .unwrap();
    let journal = g.conversation.journal(&owner).unwrap();
    journal.lock().unwrap().compact().unwrap();
    assert!(journal.lock().unwrap().submission(&cancelled.id).is_none());
    let result = snapshot(&g, &w, &owner);
    assert_eq!(result, snapshot(&g, &w, &owner));
    assert_eq!(
        result
            .submissions
            .iter()
            .map(|entry| (&entry.id, entry.state))
            .collect::<Vec<_>>(),
        vec![
            (&committed.id, SubmissionState::Committed),
            (&cancelled.id, SubmissionState::Cancelled),
            (&pending.id, SubmissionState::Pending),
        ]
    );
    assert_eq!(result.submissions[1].branch_id, "alternate");
    assert!(
        result
            .submissions
            .windows(2)
            .all(|entries| entries[0].order < entries[1].order)
    );
    assert_eq!(
        result.durable_journal_sequence,
        journal.lock().unwrap().sequence()
    );
    assert_eq!(
        result
            .branches
            .iter()
            .map(|branch| branch.branch_id.as_str())
            .collect::<Vec<_>>(),
        ["alternate", "main"]
    );
    let debug = format!("{result:?}");
    for secret in [
        "snapshot-private-body",
        "snapshot-private-principal",
        "other-conversation",
        "root-two",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn snapshot_selection_is_instance_local_and_never_uses_the_runtime_session_cursor() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let a = writer(&g, &owner);
    let b = writer(&g, &owner);
    g.select_managed_conversation(&a, &owner, CONVERSATION, "main")
        .unwrap();
    g.select_managed_conversation(&b, &owner, CONVERSATION, "alternate")
        .unwrap();
    let authority = OperationAuthority::capture(&g, &owner).unwrap();
    with_operation(&g, &authority, |runtime| {
        ensure_session_at(runtime, CONVERSATION, Some("root-one-alt"))?;
        Ok(())
    })
    .unwrap();
    let first = snapshot(&g, &a, &owner);
    let second = snapshot(&g, &b, &owner);
    assert_eq!(first.selected_branch_id.as_deref(), Some("main"));
    assert_eq!(first.selected_head_node_id.as_deref(), Some("root-one"));
    assert_eq!(second.selected_branch_id.as_deref(), Some("alternate"));
    assert_eq!(
        second.selected_head_node_id.as_deref(),
        Some("root-one-alt")
    );
    assert_eq!(first.branches, second.branches);
    assert_eq!(first.submissions, second.submissions);
    g.select_managed_conversation(&b, &owner, "conversation-two", "main")
        .unwrap();
    let unrelated = snapshot(&g, &b, &owner);
    assert_eq!(unrelated.selected_branch_id, None);
    assert_eq!(unrelated.selected_head_node_id, None);
    let before = g.instance_managed_state(&b, &owner).unwrap();
    snapshot(&g, &b, &owner);
    assert_eq!(before, g.instance_managed_state(&b, &owner).unwrap());
}

#[test]
fn snapshot_separates_finalize_won_archive_publication_and_commit_marker() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    g.set_conversation_policy(
        &w,
        &owner,
        CONVERSATION,
        crate::ConversationPolicy::VisibleAutoBranch,
    )
    .unwrap();
    let receipt = submit(&g, &w, &owner, "fork-turn", "main", "root-one");
    let journal = g.conversation.journal(&owner).unwrap();
    let submission = journal
        .lock()
        .unwrap()
        .submission(&receipt.id)
        .unwrap()
        .clone();
    let (branch_id, branch_label) =
        super::super::finalization::automatic_branch_identity(&submission).unwrap();
    let capture = FinalizeCapture {
        branch_id,
        branch_label: Some(branch_label.clone()),
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: w.principal_id().to_owned(),
        policy_revision: 2,
    };
    let revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&receipt.id.conversation_id);
    journal
        .lock()
        .unwrap()
        .claim_finalize(&receipt.id, revision, capture.clone())
        .unwrap();
    let before = snapshot(&g, &w, &owner);
    assert_eq!(before.submissions[0].state, SubmissionState::Finalizing);
    assert_eq!(before.submissions[0].branch_id, capture.branch_id);
    assert_eq!(before.branches.len(), 2);
    assert_eq!(
        (before.policy, before.policy_revision),
        (crate::ConversationPolicy::VisibleAutoBranch, 2)
    );
    let authority = OperationAuthority::capture(&g, &owner).unwrap();
    let archive = with_operation(&g, &authority, |runtime| {
        archive_receipt_or_commit(runtime, &receipt.id, &submission, &capture, &[])
    })
    .unwrap();
    let published = snapshot(&g, &w, &owner);
    assert_eq!(published.submissions[0].state, SubmissionState::Finalizing);
    assert_eq!(
        published.durable_journal_sequence,
        before.durable_journal_sequence
    );
    let branch = published
        .branches
        .iter()
        .find(|branch| branch.branch_id == capture.branch_id)
        .unwrap();
    assert_eq!(branch.head_node_id.as_deref(), Some("fork-turn"));
    assert_eq!(branch.fork_parent_node_id.as_deref(), Some("root-one"));
    assert_eq!(branch.display_label.as_deref(), Some(branch_label.as_str()));
    journal
        .lock()
        .unwrap()
        .record_archive_commit(&receipt.id, archive)
        .unwrap();
    let committed = snapshot(&g, &w, &owner);
    assert_eq!(committed.submissions[0].state, SubmissionState::Committed);
    assert!(committed.durable_journal_sequence > published.durable_journal_sequence);
    g.resume_conversation_submission(&w, &owner, CONVERSATION, "fork-turn")
        .unwrap();
    journal.lock().unwrap().compact().unwrap();
    let terminal = snapshot(&g, &w, &owner);
    assert_eq!(terminal.branches, committed.branches);
    assert_eq!(terminal.submissions[0].state, SubmissionState::Committed);
    assert!(
        terminal.submissions[0].transition_revision > committed.submissions[0].transition_revision
    );
}

#[test]
fn snapshot_generation_checkpoint_and_terminal_identity_survive_restart() {
    let state = test_path("snapshot-generation-state");
    let path = test_path("snapshot-generation.rel");
    let g = gateway_at(state.clone());
    let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let w = writer(&g, &owner);
    submit(&g, &w, &owner, "generation-turn", "main", "root-one");
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, CONVERSATION, "generation-turn")
        .unwrap()
        .status
        .generation
        .unwrap();
    let running = snapshot(&g, &w, &owner);
    assert_eq!(running.generations[0].status, GenerationStatus::Running);
    assert_eq!(running.generations[0].durable_checkpoint_sequence, None);
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "snapshot-private-prefix",
    )
    .unwrap();
    let checkpoint = snapshot(&g, &w, &owner);
    assert_eq!(
        checkpoint.generations[0].durable_checkpoint_sequence,
        Some(1)
    );
    assert!(!format!("{checkpoint:?}").contains("snapshot-private-prefix"));
    assert_eq!(checkpoint.branches, running.branches);
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g);
    let recovered = gateway_at(state);
    assert_eq!(
        recovered
            .mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
            .unwrap(),
        owner
    );
    let w = writer(&recovered, &owner);
    let interrupted = snapshot(&recovered, &w, &owner);
    assert_eq!(
        interrupted.generations[0].identity,
        checkpoint.generations[0].identity
    );
    assert_eq!(
        interrupted.generations[0].status,
        GenerationStatus::Interrupted
    );
    assert_eq!(
        interrupted.generations[0].durable_checkpoint_sequence,
        Some(1)
    );
    assert_eq!(interrupted.branches, checkpoint.branches);
}

#[test]
fn snapshot_waits_for_the_semantic_owner_before_reading_the_journal() {
    let g = Arc::new(gateway());
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let receipt = submit(&g, &w, &owner, "race-cancel", "alternate", "root-one-alt");
    let journal = g.conversation.journal(&owner).unwrap();
    let authority = OperationAuthority::capture(&g, &owner).unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let (handle, sequence) = with_operation(&g, &authority, |runtime| {
        let worker_gateway = Arc::clone(&g);
        let worker_instance = Arc::clone(&w);
        let worker_owner = owner.clone();
        let handle = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let result = snapshot(&worker_gateway, &worker_instance, &worker_owner);
            done_tx.send(result).unwrap();
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert!(done_rx.try_recv().is_err());
        // Snapshot cannot take the journal first: this semantic-owner operation
        // must still acquire the journal while the observer is waiting.
        let mut journal = journal.lock().unwrap();
        let revision = journal.conversation_revision(&receipt.id.conversation_id);
        journal.cancel(&receipt.id, revision).unwrap();
        let sequence = journal.sequence();
        drop(journal);
        runtime
            .cva
            .append_node(
                "race-head".into(),
                CONVERSATION.into(),
                Some("root-one".into()),
                "user".into(),
                9,
                "race-private-body",
            )
            .map_err(operation)?;
        runtime
            .cva
            .append_branch(Branch {
                id: "main".into(),
                conversation_id: CONVERSATION.into(),
                leaf_node_id: "race-head".into(),
                canonical: true,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)?;
        Ok((handle, sequence))
    })
    .unwrap();
    let result = done_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    handle.join().unwrap();
    assert_eq!(result.durable_journal_sequence, sequence);
    assert_eq!(result.submissions[0].state, SubmissionState::Cancelled);
    assert_eq!(
        result
            .branches
            .iter()
            .find(|branch| branch.branch_id == "main")
            .unwrap()
            .head_node_id
            .as_deref(),
        Some("race-head")
    );
}

#[test]
fn snapshot_requires_current_view_authority_and_does_not_create_sessions() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let authority = OperationAuthority::capture(&g, &owner).unwrap();
    let before = with_operation(&g, &authority, |runtime| {
        assert!(runtime.sessions.is_empty());
        Ok((
            runtime.cva().rel_turn_count(),
            runtime.cva().latest_global_version(),
            runtime.cva().archive_version(),
        ))
    })
    .unwrap();
    let empty = g
        .conversation_observation_snapshot(&w, &owner, "not-yet-created")
        .unwrap();
    assert!(empty.branches.is_empty());
    assert!(empty.submissions.is_empty());
    assert_eq!(empty.selected_branch_id, None);
    assert_eq!(empty.policy_revision, 1);
    assert_eq!(
        empty.policy,
        crate::ConversationPolicy::SequentialChronological
    );
    assert_eq!(
        with_operation(&g, &authority, |runtime| {
            assert!(runtime.sessions.is_empty());
            Ok((
                runtime.cva().rel_turn_count(),
                runtime.cva().latest_global_version(),
                runtime.cva().archive_version(),
            ))
        })
        .unwrap(),
        before
    );
    g.replace_instance_grants(&w, grants(&owner, &[OwnerGrant::SubmitConversation]))
        .unwrap();
    assert!(
        g.conversation_observation_snapshot(&w, &owner, CONVERSATION)
            .is_err()
    );
    g.replace_instance_grants(&w, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    snapshot(&g, &w, &owner);
    g.detach_instance(&w).unwrap();
    assert!(
        g.conversation_observation_snapshot(&w, &owner, CONVERSATION)
            .is_err()
    );
}

#[test]
fn snapshot_keeps_independent_generation_and_unknown_host_operation_statuses() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    submit(&g, &w, &owner, "main-generation", "main", "root-one");
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let main = g
        .start_conversation_generation(&owner, CONVERSATION, "main-generation")
        .unwrap()
        .status
        .generation
        .unwrap();
    submit(
        &g,
        &w,
        &owner,
        "alternate-generation",
        "alternate",
        "root-one-alt",
    );
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let alternate = g
        .start_conversation_generation(&owner, CONVERSATION, "alternate-generation")
        .unwrap()
        .status
        .generation
        .unwrap();
    let both = snapshot(&g, &w, &owner);
    assert_eq!(both.generations.len(), 2);
    assert_eq!(both.generations[0].identity.branch_id, "alternate");
    assert_eq!(both.generations[1].identity.branch_id, "main");
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &alternate.operation_id,
        1,
        "private-completed-prefix",
    )
    .unwrap();
    g.complete_conversation_generation(&owner, CONVERSATION, &alternate.operation_id)
        .unwrap();
    let terminal = snapshot(&g, &w, &owner);
    assert_eq!(terminal.generations[0].status, GenerationStatus::Completed);
    assert_eq!(terminal.generations[0].durable_checkpoint_sequence, Some(1));
    assert_eq!(
        terminal.generations[1].identity.operation_id,
        main.operation_id
    );
    assert_eq!(terminal.generations[1].status, GenerationStatus::Running);
    assert_eq!(
        terminal.branches[0].head_node_id.as_deref(),
        Some(alternate.generation_id.as_str())
    );

    let host = submit(
        &g,
        &w,
        &owner,
        "host-operation",
        "alternate",
        &alternate.generation_id,
    );
    let journal = g.conversation.journal(&owner).unwrap();
    let capability = RuntimeCapability::ConversationExecution;
    {
        let mut journal = journal.lock().unwrap();
        journal
            .begin_host_handoff(
                &host.id,
                capability,
                "snapshot-host-operation".into(),
                [42; 32],
            )
            .unwrap();
        journal
            .record_host_status(&host.id, capability, HostOperationStatus::Running)
            .unwrap();
        journal.begin_host_cancel(&host.id, capability).unwrap();
        journal
            .record_host_status(&host.id, capability, HostOperationStatus::Unknown)
            .unwrap();
    }
    let unknown = snapshot(&g, &w, &owner);
    assert_eq!(unknown.host_operations.len(), 1);
    assert_eq!(
        unknown.host_operations[0].phase,
        DurableHostOperationPhase::Unknown
    );
    assert!(unknown.host_operations[0].cancel_requested);
    assert_eq!(
        unknown.host_operations[0].operation_id,
        "snapshot-host-operation"
    );
    assert_eq!(unknown.host_operations[0].submission_id, host.id);
    assert!(!format!("{unknown:?}").contains("payload_digest"));
    journal
        .lock()
        .unwrap()
        .record_host_status(&host.id, capability, HostOperationStatus::Cancelled)
        .unwrap();
    let cancelled = snapshot(&g, &w, &owner);
    assert_eq!(
        cancelled.host_operations[0].phase,
        DurableHostOperationPhase::Cancelled
    );
    assert!(
        cancelled.host_operations[0].transition_revision
            > unknown.host_operations[0].transition_revision
    );
}

#[test]
fn checkpoint_reader_truncates_large_ahead_text_without_reading_session_records() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    submit(&g, &w, &owner, "bounded-generation", "main", "root-one");
    g.process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    let generation = g
        .start_conversation_generation(&owner, CONVERSATION, "bounded-generation")
        .unwrap()
        .status
        .generation
        .unwrap();
    let prefix = "durable 🌱 prefix";
    g.checkpoint_conversation_generation(&owner, CONVERSATION, &generation.operation_id, 1, prefix)
        .unwrap();

    let authority = OperationAuthority::capture(&g, &owner).unwrap();
    with_operation(&g, &authority, |runtime| {
        let mut ahead = runtime
            .cva()
            .interaction_stream_record(&generation.generation_id)
            .cloned()
            .ok_or_else(|| operation("generation stream is missing"))?;
        ahead.content.push_str("AHEAD_SENTINEL");
        ahead.content.push_str(&"x".repeat(1_100_000));
        runtime
            .cva
            .put_interaction_stream(ahead)
            .map_err(operation)?;
        runtime
            .cva
            .put_interaction_stream(InteractionStreamRecord {
                message_id: "unrelated-large-stream".into(),
                session_id: CONVERSATION.into(),
                parent_message_id: Some(generation.generation_id.clone()),
                role: InteractionRole::Agent,
                timestamp_ns: generation.started_timestamp_ns + 1,
                content: format!("UNRELATED_SENTINEL{}", "y".repeat(1_100_000)),
                status: InteractionStreamStatus::Streaming,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)?;
        Ok(())
    })
    .unwrap();

    let observed = snapshot(&g, &w, &owner);
    let generation_observation = observed
        .generations
        .iter()
        .find(|observed| observed.identity.generation_id == generation.generation_id)
        .unwrap();
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    let checkpoint = g
        .read_conversation_checkpoint(
            &owner,
            token.mount_epoch,
            &ConversationId::new(CONVERSATION).unwrap(),
            generation_observation,
        )
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.text, prefix);
    assert!(!checkpoint.text.contains("AHEAD_SENTINEL"));
    assert!(!checkpoint.text.contains("UNRELATED_SENTINEL"));
}

#[test]
fn broad_source_buckets_journal_statuses_by_conversation() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let first = submit(&g, &w, &owner, "first-pending", "main", "root-one");
    let second = g
        .submit_conversation(
            &w,
            &owner,
            submission("conversation-two", "second-pending", "root-two"),
        )
        .unwrap();

    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    let source = g
        .read_conversation_distribution_source(&owner, token.mount_epoch, None)
        .unwrap();
    let first_conversation = source
        .conversations
        .iter()
        .find(|entry| entry.conversation_id.as_str() == CONVERSATION)
        .unwrap();
    let second_conversation = source
        .conversations
        .iter()
        .find(|entry| entry.conversation_id.as_str() == "conversation-two")
        .unwrap();
    assert_eq!(
        first_conversation
            .submissions
            .iter()
            .map(|entry| &entry.id)
            .collect::<Vec<_>>(),
        [&first.id]
    );
    assert_eq!(
        second_conversation
            .submissions
            .iter()
            .map(|entry| &entry.id)
            .collect::<Vec<_>>(),
        [&second.id]
    );
}
