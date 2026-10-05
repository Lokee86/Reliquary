use super::*;

fn writer(gateway: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    gateway
        .attach_instance(
            "sequential-principal".into(),
            None,
            grants(
                owner,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap()
}

#[test]
fn selected_branch_parent_mismatch_is_rejected_before_durable_admission() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    for (conversation, parent) in [
        ("conversation-one", "root-one-alt"),
        ("conversation-one", "root-two"),
    ] {
        assert!(
            gateway
                .submit_conversation(
                    &writer,
                    &owner,
                    submission(conversation, "wrong-branch", parent)
                )
                .is_err()
        );
    }
    assert!(
        gateway
            .conversation
            .journal(&owner)
            .unwrap()
            .lock()
            .unwrap()
            .statuses()
            .is_empty()
    );
}

#[test]
fn queued_sequential_turns_capture_current_branch_head_without_provisional_effects() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    let first = gateway
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "sequential-first", "root-one"),
        )
        .unwrap();
    let mut request = submission("conversation-one", "sequential-second", "root-one");
    request.expected_conversation_revision = first.status.transition_revision;
    gateway
        .submit_conversation(&writer, &owner, request)
        .unwrap();
    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    let initial = with_operation(&gateway, &authority, |runtime| {
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "sequential-first")
        );
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "sequential-second")
        );
        assert!(runtime.cva().episodes().is_empty());
        assert_eq!(
            runtime
                .cva()
                .activity_position_for_turn("conversation-one", "sequential-first"),
            None
        );
        // An unrelated ambient cursor cannot choose the finalization parent.
        ensure_session_at(runtime, "conversation-one", Some("root-one-alt"))?;
        Ok(runtime.cva().rel_turn_count())
    })
    .unwrap();
    let first = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    let second = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("root-one")
    );
    let capture = second.status.finalize_capture.unwrap();
    assert_eq!(capture.branch_id, "main");
    assert_eq!(capture.parent_node_id.as_deref(), Some("sequential-first"));
    assert_eq!(capture.policy_revision, 1);
    with_operation(&gateway, &authority, |runtime| {
        assert_eq!(runtime.cva().rel_turn_count(), initial + 2);
        assert_eq!(
            runtime
                .cva()
                .archive()
                .nodes
                .get("conversation-one", "sequential-second")
                .unwrap()
                .parent_id
                .as_deref(),
            Some("sequential-first")
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn unrelated_admitted_parent_at_dispatch_leaves_submission_pending() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    let key = ConversationSubmissionId {
        conversation_id: ConversationId::new("conversation-one").unwrap(),
        submission_id: SubmissionId::new("unrelated-parent").unwrap(),
    };
    // A legacy retained queue can contain a parent from a different branch.
    // Fresh API admission rejects it; dispatch must also fail closed.
    let content = "legacy unrelated parent".to_owned();
    let entry = Submission {
        id: key.clone(),
        principal_id: writer.principal_id().to_owned(),
        scope_fingerprint: fingerprint(&json!({
            "rel_id": owner, "conversation_id": "conversation-one", "branch_id": "main",
        }))
        .unwrap(),
        payload_hash: Submission::canonical_payload_hash(&content, &[]).unwrap(),
        request_metadata: json!({"branch_id": "main", "expected_parent_node_id": "root-one-alt"}),
        content,
        attachments: vec![],
        client_timestamp_ns: None,
    };
    let journal = gateway.conversation.journal(&owner).unwrap();
    journal.lock().unwrap().enqueue(entry, now_ns(), 0).unwrap();
    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    let before = with_operation(&gateway, &authority, |runtime| {
        Ok(runtime.cva().rel_turn_count())
    })
    .unwrap();
    assert!(
        gateway
            .process_next_conversation_submission(&writer, &owner, "conversation-one")
            .is_err()
    );
    let status = journal.lock().unwrap().status(&key).unwrap();
    assert_eq!(status.state, SubmissionState::Pending);
    assert!(status.finalize_capture.is_none());
    with_operation(&gateway, &authority, |runtime| {
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "unrelated-parent")
        );
        assert_eq!(runtime.cva().rel_turn_count(), before);
        Ok(())
    })
    .unwrap();
}

#[test]
fn captured_finalization_cannot_rewind_an_advanced_archive_branch() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    let receipt = gateway
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "frozen-head", "root-one"),
        )
        .unwrap();
    let journal = gateway.conversation.journal(&owner).unwrap();
    let submission = journal
        .lock()
        .unwrap()
        .submission(&receipt.id)
        .unwrap()
        .clone();
    let capture = FinalizeCapture {
        branch_id: "main".into(),
        branch_label: None,
        parent_node_id: Some("root-one".into()),
        committed_timestamp_ns: now_ns(),
        verified_principal_id: submission.principal_id.clone(),
        policy_revision: 1,
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
    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &authority, |runtime| {
        runtime
            .cva
            .append_node(
                "external-advance".into(),
                "conversation-one".into(),
                Some("root-one".into()),
                "user".into(),
                3,
                "independent committed turn",
            )
            .map_err(operation)?;
        runtime
            .cva
            .append_branch(Branch {
                id: "main".into(),
                conversation_id: "conversation-one".into(),
                leaf_node_id: "external-advance".into(),
                canonical: true,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)?;
        assert!(
            archive_receipt_or_commit(runtime, &receipt.id, &submission, &capture, &[]).is_err()
        );
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", "frozen-head")
        );
        // Simulate a synced stable node left by an interrupted publisher.
        runtime
            .accept_turn(InteractionTurn {
                message_id: "frozen-head".into(),
                session_id: "conversation-one".into(),
                parent_message_id: capture.parent_node_id.clone(),
                role: InteractionRole::User,
                principal_id: Some(submission.principal_id.clone()),
                timestamp_ns: capture.committed_timestamp_ns,
                content: submission.content.clone(),
                attachments: vec![],
                project_attachments: vec![],
            })
            .map_err(operation)?;
        assert!(archive_receipt_if_present(runtime, &receipt.id, &submission, &capture).is_err());
        assert!(
            archive_receipt_or_commit(runtime, &receipt.id, &submission, &capture, &[]).is_err()
        );
        assert_eq!(
            runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .find(|branch| branch.conversation_id == "conversation-one" && branch.id == "main")
                .unwrap()
                .leaf_node_id,
            "external-advance"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        journal.lock().unwrap().status(&receipt.id).unwrap().state,
        SubmissionState::Finalizing
    );
}

#[test]
fn restart_at_each_finalization_handoff_preserves_one_turn_activity_and_episode_work() {
    // 0 admission; 1 FinalizeWon; 2 synced Node before branch; 3 synced branch;
    // 4 Archive receipt; 5 scheduled effects before marker; 6 postcommit marker.
    for cut in 0..7 {
        let state = test_path("sequential-crash-state");
        let path = test_path("sequential-crash.rel");
        let mut first = gateway_at(state.clone());
        first.episode_policy.episode.max_input_bytes = 1;
        let owner = first.mount_rel(seeded_runtime_at(path.clone())).unwrap();
        let writer = writer(&first, &owner);
        let receipt = first
            .submit_conversation(
                &writer,
                &owner,
                submission("conversation-one", "crash-turn", "root-one"),
            )
            .unwrap();
        first
            .submit_conversation(
                &writer,
                &owner,
                submission("conversation-two", "provisional-turn", "root-two"),
            )
            .unwrap();
        let journal = first.conversation.journal(&owner).unwrap();
        let submission = journal
            .lock()
            .unwrap()
            .submission(&receipt.id)
            .unwrap()
            .clone();
        let capture = FinalizeCapture {
            branch_id: "main".into(),
            branch_label: None,
            parent_node_id: Some("root-one".into()),
            committed_timestamp_ns: now_ns(),
            verified_principal_id: submission.principal_id.clone(),
            policy_revision: 1,
        };
        if cut >= 1 {
            let revision = journal
                .lock()
                .unwrap()
                .conversation_revision(&receipt.id.conversation_id);
            journal
                .lock()
                .unwrap()
                .claim_finalize(&receipt.id, revision, capture.clone())
                .unwrap();
        }
        let authority = OperationAuthority::capture(&first, &owner).unwrap();
        let initial_count = with_operation(&first, &authority, |runtime| {
            Ok(runtime.cva().rel_turn_count())
        })
        .unwrap();
        if cut == 2 {
            with_operation(&first, &authority, |runtime| {
                runtime
                    .accept_turn(InteractionTurn {
                        message_id: "crash-turn".into(),
                        session_id: "conversation-one".into(),
                        parent_message_id: capture.parent_node_id.clone(),
                        role: InteractionRole::User,
                        principal_id: Some(submission.principal_id.clone()),
                        timestamp_ns: capture.committed_timestamp_ns,
                        content: submission.content.clone(),
                        attachments: vec![],
                        project_attachments: vec![],
                    })
                    .map_err(operation)?;
                Ok(())
            })
            .unwrap();
        }
        if cut >= 3 {
            let archive = with_operation(&first, &authority, |runtime| {
                archive_receipt_or_commit(runtime, &receipt.id, &submission, &capture, &[])
            })
            .unwrap();
            if cut >= 4 {
                journal
                    .lock()
                    .unwrap()
                    .record_archive_commit(&receipt.id, archive)
                    .unwrap();
            }
        }
        if cut >= 5 {
            with_operation(&first, &authority, |runtime| {
                ensure_session_at(runtime, "conversation-one", Some("crash-turn"))?;
                runtime
                    .schedule_session("conversation-one", first.episode_policy, now_ns())
                    .map_err(operation)?;
                Ok(())
            })
            .unwrap();
        }
        let scheduled =
            with_operation(&first, &authority, |runtime| Ok(runtime.cva().episodes())).unwrap();
        if cut == 6 {
            first
                .resume_conversation_submission(&writer, &owner, "conversation-one", "crash-turn")
                .unwrap();
        }
        // Gateway Drop normally runs orderly shutdown/reconciliation. Remove this
        // test owner first so closing handles preserves the injected crash cut.
        let execution = first
            .rel_executions
            .write()
            .unwrap()
            .remove(&owner)
            .unwrap();
        drop(execution);
        drop(journal);
        drop(writer);
        drop(first);
        let mut recovered = gateway_at(state);
        recovered.episode_policy.episode.max_input_bytes = 1;
        assert_eq!(
            recovered
                .mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
                .unwrap(),
            owner
        );
        let writer = self::writer(&recovered, &owner);
        if cut <= 1 {
            recovered
                .process_next_conversation_submission(&writer, &owner, "conversation-one")
                .unwrap()
                .unwrap();
        }
        let status = recovered
            .resume_conversation_submission(&writer, &owner, "conversation-one", "crash-turn")
            .unwrap()
            .status;
        assert_eq!(status.state, SubmissionState::Committed, "cut {cut}");
        assert!(status.post_commit_receipt.is_some());
        if cut >= 1 {
            assert_eq!(status.finalize_capture.as_ref(), Some(&capture));
        }
        let authority = OperationAuthority::capture(&recovered, &owner).unwrap();
        with_operation(&recovered, &authority, |runtime| {
            assert_eq!(
                runtime.cva().rel_turn_count(),
                initial_count + 1,
                "cut {cut}"
            );
            assert_eq!(
                runtime
                    .cva()
                    .activity_position_for_turn("conversation-one", "crash-turn"),
                Some(initial_count + 1)
            );
            assert!(
                !runtime
                    .cva()
                    .archive()
                    .has_node("conversation-two", "provisional-turn")
            );
            assert_eq!(
                runtime
                    .cva()
                    .activity_position_for_turn("conversation-two", "provisional-turn"),
                None
            );
            assert!(
                runtime
                    .cva()
                    .episodes_for_conversation("conversation-two")
                    .is_empty()
            );
            assert!(
                !runtime
                    .cva()
                    .episodes_for_conversation("conversation-one")
                    .is_empty()
            );
            if cut >= 5 {
                assert_eq!(runtime.cva().episodes(), scheduled, "cut {cut}");
            }
            let episodes = runtime.cva().episodes();
            let work = runtime.cva().insomnia_stats();
            let version = runtime.cva().archive_version();
            ensure_session_at(runtime, "conversation-one", Some("crash-turn"))?;
            runtime
                .schedule_session("conversation-one", recovered.episode_policy, now_ns())
                .map_err(operation)?;
            assert_eq!(runtime.cva().episodes(), episodes);
            assert_eq!(runtime.cva().insomnia_stats(), work);
            assert_eq!(runtime.cva().archive_version(), version);
            Ok(())
        })
        .unwrap();
        assert!(
            recovered
                .process_next_conversation_submission(&writer, &owner, "conversation-one")
                .unwrap()
                .is_none()
        );
        recovered.detach_instance(&writer).unwrap();
        drop(recovered.unmount_rel(&owner).unwrap());
    }
}

#[test]
fn committed_retry_survives_advanced_head_and_compaction_but_cannot_change_request() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    let request = submission("conversation-one", "retry-committed", "root-one");
    gateway
        .submit_conversation(&writer, &owner, request.clone())
        .unwrap();
    let committed = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    gateway
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .compact()
        .unwrap();
    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    let version = with_operation(&gateway, &authority, |runtime| {
        Ok(runtime.cva().archive_version())
    })
    .unwrap();
    let retry = gateway
        .submit_conversation(&writer, &owner, request.clone())
        .unwrap();
    assert_eq!(retry.id, committed.id);
    assert_eq!(retry.status.state, SubmissionState::Committed);
    assert_eq!(
        retry.status.archive_receipt,
        committed.status.archive_receipt
    );
    assert_eq!(
        retry.status.post_commit_receipt,
        committed.status.post_commit_receipt
    );
    let mut changed = request.clone();
    changed.expected_parent_node_id = Some("root-one-alt".into());
    assert!(
        gateway
            .submit_conversation(&writer, &owner, changed)
            .is_err()
    );
    let mut changed = request;
    changed.content.push_str(" changed");
    assert!(
        gateway
            .submit_conversation(&writer, &owner, changed)
            .is_err()
    );
    with_operation(&gateway, &authority, |runtime| {
        assert_eq!(runtime.cva().archive_version(), version);
        Ok(())
    })
    .unwrap();
}

#[test]
fn selected_branch_head_remains_valid_when_another_branch_has_extended_it() {
    let gateway = gateway();
    let owner = gateway.mount_rel(seeded_runtime()).unwrap();
    let writer = writer(&gateway, &owner);
    let authority = OperationAuthority::capture(&gateway, &owner).unwrap();
    with_operation(&gateway, &authority, |runtime| {
        runtime
            .cva
            .append_node(
                "fork-child".into(),
                "conversation-one".into(),
                Some("root-one".into()),
                "user".into(),
                3,
                "other branch extension",
            )
            .map_err(operation)?;
        runtime
            .cva
            .append_branch(Branch {
                id: "fork".into(),
                conversation_id: "conversation-one".into(),
                leaf_node_id: "fork-child".into(),
                canonical: false,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)?;
        Ok(())
    })
    .unwrap();
    gateway
        .submit_conversation(
            &writer,
            &owner,
            submission("conversation-one", "shared-prefix", "root-one"),
        )
        .unwrap();
    let committed = gateway
        .process_next_conversation_submission(&writer, &owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(
        committed
            .status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some("root-one")
    );
}
