use super::*;
use crate::multiplexing_journal::DurableGenerationPhase;

fn writer(g: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    g.attach_instance(
        "generation-principal".into(),
        None,
        grants(
            owner,
            &[
                OwnerGrant::ViewRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelOwnPendingSubmission,
                OwnerGrant::CancelSharedGeneration,
            ],
        ),
    )
    .unwrap()
}

fn begin(g: &GatewayRuntime, w: &InstanceRuntime, owner: &str) -> GatewaySubmissionReceipt {
    g.submit_conversation(
        w,
        owner,
        submission("conversation-one", "generation-user", "root-one"),
    )
    .unwrap();
    g.process_next_accepted_conversation_submission(owner, "conversation-one")
        .unwrap()
        .unwrap();
    g.start_conversation_generation(owner, "conversation-one", "generation-user")
        .unwrap()
}

#[test]
fn generation_survives_zero_viewers_and_blocks_only_its_branch() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let viewer = g
        .attach_conversation_viewer(&w, &owner, "conversation-one")
        .unwrap();
    let started = begin(&g, &w, &owner);
    let generation = started.status.generation.unwrap();
    assert_eq!(generation.phase, DurableGenerationPhase::Running);
    let mut pending = submission("conversation-one", "after-generation", "generation-user");
    pending.expected_conversation_revision = g
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&started.id.conversation_id);
    g.submit_conversation(&w, &owner, pending).unwrap();
    let mut other = submission(
        "conversation-one",
        "independent-generation-branch",
        "root-one-alt",
    );
    other.branch_id = "alternate".into();
    other.expected_conversation_revision = g
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .conversation_revision(&started.id.conversation_id);
    g.submit_conversation(&w, &owner, other).unwrap();
    g.detach_conversation_viewer(viewer).unwrap();
    g.detach_instance(&w).unwrap();
    let other = g
        .process_next_accepted_conversation_submission(&owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(
        other.id.submission_id.as_str(),
        "independent-generation-branch"
    );
    assert!(
        g.process_next_accepted_conversation_submission(&owner, "conversation-one")
            .unwrap()
            .is_none()
    );
    g.checkpoint_conversation_generation(
        &owner,
        "conversation-one",
        &generation.operation_id,
        1,
        "durable reply",
    )
    .unwrap();
    let late = writer(&g, &owner);
    let status = g
        .conversation_generation_status(&late, &owner, "conversation-one", &generation.operation_id)
        .unwrap();
    assert_eq!(
        status
            .status
            .generation
            .as_ref()
            .unwrap()
            .checkpoint
            .as_ref()
            .unwrap()
            .sequence,
        1
    );
    g.complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
        .unwrap();
    let next = g
        .process_next_accepted_conversation_submission(&owner, "conversation-one")
        .unwrap()
        .unwrap();
    assert_eq!(
        next.status
            .finalize_capture
            .unwrap()
            .parent_node_id
            .as_deref(),
        Some(generation.generation_id.as_str())
    );
    g.detach_instance(&late).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn generation_checkpoints_are_monotonic_and_completion_is_exactly_once() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let generation = begin(&g, &w, &owner).status.generation.unwrap();
    for (sequence, text) in [(1, "hello"), (1, "hello"), (2, "hello world")] {
        g.checkpoint_conversation_generation(
            &owner,
            "conversation-one",
            &generation.operation_id,
            sequence,
            text,
        )
        .unwrap();
    }
    for (sequence, text) in [
        (2, "changed"),
        (3, "rewritten content"),
        (4, "hello world!"),
    ] {
        assert!(
            g.checkpoint_conversation_generation(
                &owner,
                "conversation-one",
                &generation.operation_id,
                sequence,
                text
            )
            .is_err()
        );
    }
    let completed = g
        .complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
        .unwrap();
    assert_eq!(
        completed.status.generation.unwrap().phase,
        DurableGenerationPhase::Completed
    );
    g.complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
        .unwrap();
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    g.with_rel(&token, |runtime| {
        let turns = runtime
            .conversation_turns("conversation-one", &generation.generation_id)
            .unwrap();
        assert_eq!(
            turns
                .iter()
                .filter(|turn| turn.node_id == generation.generation_id)
                .count(),
            1
        );
        assert_eq!(turns.last().unwrap().content, "hello world");
        Ok(())
    })
    .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn generation_control_requires_current_shared_cancel_grant() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let generation = begin(&g, &w, &owner).status.generation.unwrap();
    let own = g
        .attach_instance(
            "generation-principal".into(),
            None,
            grants(
                &owner,
                &[OwnerGrant::ViewRel, OwnerGrant::CancelOwnPendingSubmission],
            ),
        )
        .unwrap();
    assert!(
        g.cancel_conversation_generation(
            &own,
            &owner,
            "conversation-one",
            &generation.operation_id
        )
        .is_err()
    );
    assert!(
        g.cancel_conversation_submission(&own, &owner, "conversation-one", "generation-user")
            .is_err()
    );
    g.checkpoint_conversation_generation(
        &owner,
        "conversation-one",
        &generation.operation_id,
        1,
        "partial",
    )
    .unwrap();
    let cancelled = g
        .cancel_conversation_generation(&w, &owner, "conversation-one", &generation.operation_id)
        .unwrap();
    assert_eq!(
        cancelled.status.generation.unwrap().phase,
        DurableGenerationPhase::Interrupted
    );
    assert!(
        g.complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
            .is_err()
    );
    g.detach_instance(&own).unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn generation_restart_recovers_interrupted_checkpoint_and_stable_identity() {
    let state = test_path("generation-restart-state");
    let path = test_path("generation-restart.rel");
    let g = gateway_at(state.clone());
    let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
    let w = writer(&g, &owner);
    let generation = begin(&g, &w, &owner).status.generation.unwrap();
    g.checkpoint_conversation_generation(
        &owner,
        "conversation-one",
        &generation.operation_id,
        1,
        "saved prefix",
    )
    .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g);
    let g = gateway_at(state);
    let reopened = g
        .mount_rel(InteractionRuntime::new(Cva::open(&path).unwrap()))
        .unwrap();
    assert_eq!(owner, reopened);
    let w = writer(&g, &owner);
    let status = g
        .conversation_generation_status(&w, &owner, "conversation-one", &generation.operation_id)
        .unwrap();
    let recovered = status.status.generation.unwrap();
    assert_eq!(recovered.generation_id, generation.generation_id);
    assert_eq!(recovered.phase, DurableGenerationPhase::Interrupted);
    assert_eq!(recovered.checkpoint.unwrap().sequence, 1);
    let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
    g.with_rel(&token, |runtime| {
        let records = runtime
            .cva()
            .interaction_streams_for_session("conversation-one");
        let record = records
            .iter()
            .find(|r| r.message_id == generation.generation_id)
            .unwrap();
        assert_eq!(record.content, "saved prefix");
        assert_eq!(record.status, crate::InteractionStreamStatus::Interrupted);
        assert!(
            !runtime
                .cva()
                .archive()
                .has_node("conversation-one", &generation.generation_id)
        );
        Ok(())
    })
    .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn generation_recovery_reconciles_checkpoint_and_archive_handoffs() {
    for archive_won in [false, true] {
        let state = test_path("generation-handoff-state");
        let path = test_path("generation-handoff.rel");
        let g = gateway_at(state.clone());
        let owner = g.mount_rel(seeded_runtime_at(path.clone())).unwrap();
        let w = writer(&g, &owner);
        let generation = begin(&g, &w, &owner).status.generation.unwrap();
        g.checkpoint_conversation_generation(
            &owner,
            "conversation-one",
            &generation.operation_id,
            1,
            "prefix",
        )
        .unwrap();
        let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel(&token, |runtime| {
            if archive_won {
                runtime
                    .cva
                    .append_node_with_principal(
                        generation.generation_id.clone(),
                        "conversation-one".into(),
                        Some(generation.parent_node_id.clone()),
                        "assistant".into(),
                        Some("generation-principal".into()),
                        generation.started_timestamp_ns,
                        "prefix",
                    )
                    .unwrap();
            } else {
                runtime
                    .cva
                    .put_interaction_stream(crate::InteractionStreamRecord {
                        message_id: generation.generation_id.clone(),
                        session_id: "conversation-one".into(),
                        parent_message_id: Some(generation.parent_node_id.clone()),
                        role: InteractionRole::Agent,
                        timestamp_ns: generation.started_timestamp_ns,
                        content: "prefix synced before journal".into(),
                        status: crate::InteractionStreamStatus::Streaming,
                    })
                    .unwrap();
            }
            runtime.cva.sync().unwrap();
            Ok(())
        })
        .unwrap();
        g.detach_instance(&w).unwrap();
        drop(g.unmount_rel(&owner).unwrap());
        drop(g);
        let g = gateway_at(state);
        g.mount_rel(InteractionRuntime::new(Cva::open(path).unwrap()))
            .unwrap();
        let w = writer(&g, &owner);
        let receipt = g
            .conversation_generation_status(
                &w,
                &owner,
                "conversation-one",
                &generation.operation_id,
            )
            .unwrap();
        let recovered = receipt.status.generation.unwrap();
        assert_eq!(
            recovered.phase,
            if archive_won {
                DurableGenerationPhase::Completed
            } else {
                DurableGenerationPhase::Interrupted
            }
        );
        assert_eq!(
            recovered.checkpoint.unwrap().sequence,
            if archive_won { 1 } else { 2 }
        );
        let token = g.authorize(&w, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel(&token, |runtime| {
            let branches = runtime.cva().archive().branches();
            let branch = branches
                .iter()
                .find(|b| b.conversation_id == "conversation-one" && b.id == "main")
                .unwrap();
            assert_eq!(
                branch.leaf_node_id,
                if archive_won {
                    generation.generation_id.clone()
                } else {
                    generation.parent_node_id.clone()
                }
            );
            Ok(())
        })
        .unwrap();
        g.detach_instance(&w).unwrap();
        drop(g.unmount_rel(&owner).unwrap());
    }
}

#[test]
fn generation_branch_lease_fences_overlapping_control() {
    let g = gateway();
    let owner = g.mount_rel(seeded_runtime()).unwrap();
    let w = writer(&g, &owner);
    let started = begin(&g, &w, &owner);
    let generation = started.status.generation.unwrap();
    let journal = g.conversation.journal(&owner).unwrap();
    let coordinator = g
        .conversation
        .coordinator(&owner, "conversation-one")
        .unwrap();
    let guard = coordinator.acquire(&journal, &started.id, "main").unwrap();
    assert!(
        g.checkpoint_conversation_generation(
            &owner,
            "conversation-one",
            &generation.operation_id,
            1,
            "blocked"
        )
        .is_err()
    );
    assert!(
        g.cancel_conversation_generation(&w, &owner, "conversation-one", &generation.operation_id)
            .is_err()
    );
    assert!(
        g.complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
            .is_err()
    );
    drop(guard);
    g.replace_instance_grants(&w, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    assert!(
        g.cancel_conversation_generation(&w, &owner, "conversation-one", &generation.operation_id)
            .is_err()
    );
    g.checkpoint_conversation_generation(
        &owner,
        "conversation-one",
        &generation.operation_id,
        1,
        "still owned",
    )
    .unwrap();
    g.complete_conversation_generation(&owner, "conversation-one", &generation.operation_id)
        .unwrap();
    g.detach_instance(&w).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
}
