use super::observation::ConversationTransition;
use super::streams::{StreamDelivery, StreamKey, StreamPayload};
use super::subscriptions::RelDelivery;
use super::*;
use crate::multiplexing_journal::{
    ConversationId, DurableGenerationPhase, RelOwnerId, SubmissionState,
};
use crate::runtime_host_test_support::test_path;
use crate::{Branch, ConversationPolicy, Cva, GatewayConversationSubmission, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

const CONVERSATION: &str = "conversation-one";

fn seeded(path: PathBuf) -> InteractionRuntime {
    let mut cva = Cva::create_project(path).unwrap();
    for (conversation_id, node_id, content) in [
        (CONVERSATION, "root-one", "first durable turn"),
        ("conversation-two", "root-two", "second durable turn"),
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
    }
    cva.sync().unwrap();
    InteractionRuntime::new(cva)
}

fn gateway_at(state: PathBuf) -> GatewayRuntime {
    GatewayRuntime::new(
        state,
        Default::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        Default::default(),
    )
}

fn grants(owner: &str, caps: &[OwnerGrant]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(owner.to_owned(), caps.iter().copied().collect())])
}

fn attach_full(g: &GatewayRuntime, owner: &str) -> Arc<InstanceRuntime> {
    g.attach_instance(
        "convergence-writer".into(),
        None,
        grants(
            owner,
            &[
                OwnerGrant::ViewRel,
                OwnerGrant::ManageRel,
                OwnerGrant::SubmitConversation,
                OwnerGrant::CancelOwnPendingSubmission,
                OwnerGrant::CancelSharedGeneration,
            ],
        ),
    )
    .unwrap()
}

fn request(id: &str, body: &str, parent: &str) -> GatewayConversationSubmission {
    GatewayConversationSubmission {
        conversation_id: CONVERSATION.into(),
        submission_id: id.into(),
        expected_conversation_revision: 0,
        expected_parent_node_id: Some(parent.into()),
        branch_id: "main".into(),
        content: body.into(),
        attachments: vec![],
        client_timestamp_ns: None,
    }
}

fn setup(label: &str) -> (GatewayRuntime, String, Arc<InstanceRuntime>) {
    let g = gateway_at(test_path(&format!("{label}-state")));
    let owner = g
        .mount_rel(seeded(test_path(&format!("{label}.rel"))))
        .unwrap();
    let writer = attach_full(&g, &owner);
    (g, owner, writer)
}

fn stream_key(
    g: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    generation: &crate::multiplexing_journal::DurableGeneration,
) -> StreamKey {
    let token = g.authorize(writer, owner, OwnerGrant::ViewRel).unwrap();
    StreamKey {
        rel_id: RelOwnerId::parse_canonical(owner).unwrap(),
        mount_epoch: token.mount_epoch,
        conversation_id: ConversationId::new(CONVERSATION).unwrap(),
        operation_id: generation.operation_id.clone(),
        generation_id: generation.generation_id.clone(),
        branch_id: generation.branch_id.clone(),
    }
}

fn start(
    g: &GatewayRuntime,
    writer: &InstanceRuntime,
    owner: &str,
    id: &str,
) -> crate::multiplexing_journal::DurableGeneration {
    g.submit_conversation(
        writer,
        owner,
        request(id, "submitted private body", "root-one"),
    )
    .unwrap();
    let accepted = g
        .process_next_accepted_conversation_submission(owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert_eq!(accepted.status.state, SubmissionState::Committed);
    g.start_conversation_generation(owner, CONVERSATION, id)
        .unwrap()
        .status
        .generation
        .unwrap()
}

fn broad_events(
    g: &GatewayRuntime,
    writer: &InstanceRuntime,
    sub: &super::snapshots::RelSubscription,
) -> Vec<Arc<super::events::RelEventEnvelope>> {
    let mut events = Vec::new();
    loop {
        match g.next_rel_event(writer, sub).unwrap() {
            None => break,
            Some(RelDelivery::Event(event)) => events.push(event),
            Some(other) => panic!("unexpected broad-feed resync: {other:?}"),
        }
    }
    events
}

fn stream_event(
    g: &GatewayRuntime,
    writer: &InstanceRuntime,
    sub: &super::streams::StreamSubscription,
) -> Arc<super::streams::StreamEvent> {
    match g.next_conversation_stream(writer, sub).unwrap().unwrap() {
        StreamDelivery::Event(event) => event,
        other => panic!("expected exact-stream event, got {other:?}"),
    }
}

#[test]
fn admission_policy_finalize_and_archive_converge_without_checkpoint_text_in_broad_feed() {
    let (g, owner, writer) = setup("convergence-finalize");
    let (initial, broad) = g.subscribe_rel(&writer, &owner, None, 64).unwrap();
    assert_eq!(initial.state.lifecycle, OwnerAdmission::Open);

    let changed = g
        .set_conversation_policy(
            &writer,
            &owner,
            CONVERSATION,
            ConversationPolicy::VisibleAutoBranch,
        )
        .unwrap();
    assert!(changed);
    let queued = g
        .submit_conversation(
            &writer,
            &owner,
            request("admit-turn", "private submitted body", "root-one"),
        )
        .unwrap();
    assert_eq!(queued.status.state, SubmissionState::Pending);

    let accepted = g
        .process_next_accepted_conversation_submission(&owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert_eq!(accepted.status.state, SubmissionState::Committed);
    let archive_receipt = accepted.status.archive_receipt.as_ref().unwrap();
    let committed_node_id = archive_receipt.node_id.clone();
    let committed_branch_id = archive_receipt.branch_id.clone();
    let generation = g
        .start_conversation_generation(&owner, CONVERSATION, "admit-turn")
        .unwrap()
        .status
        .generation
        .unwrap();
    assert_eq!(generation.phase, DurableGenerationPhase::Running);

    let exact_key = stream_key(&g, &writer, &owner, &generation);
    let (running_snapshot, exact) = g
        .subscribe_authoritative_conversation_stream(&writer, exact_key, 8)
        .unwrap();
    assert_eq!(
        running_snapshot.source.conversation.generations[0].status,
        super::observation::GenerationStatus::Running
    );
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "CHECKPOINT_PRIVATE_SENTINEL",
    )
    .unwrap();

    let checkpoint = stream_event(&g, &writer, &exact);
    match &checkpoint.payload {
        StreamPayload::Checkpoint(content) => {
            assert_eq!(content.text, "CHECKPOINT_PRIVATE_SENTINEL");
            assert_eq!(content.observation.durable_checkpoint_sequence, 1);
        }
        other => panic!("expected durable checkpoint, got {other:?}"),
    }
    let completed = g
        .complete_conversation_generation(&owner, CONVERSATION, &generation.operation_id)
        .unwrap();
    assert_eq!(
        completed.status.generation.unwrap().phase,
        DurableGenerationPhase::Completed
    );
    let completed_event = stream_event(&g, &writer, &exact);
    match &completed_event.payload {
        StreamPayload::Checkpoint(content) => {
            assert_eq!(
                content.observation.status,
                super::observation::GenerationStatus::Completed
            );
        }
        StreamPayload::Status(status) => assert_eq!(
            status.status,
            super::observation::GenerationStatus::Completed
        ),
    }

    let events = broad_events(&g, &writer, &broad);
    assert!(
        events
            .iter()
            .any(|event| event.class == super::events::RelEventClass::SubmissionQueueChanged)
    );
    assert!(
        events
            .iter()
            .any(|event| event.class == super::events::RelEventClass::ConversationOperationChanged)
    );
    assert!(
        events
            .iter()
            .any(|event| event.class == super::events::RelEventClass::ArchiveChanged)
    );
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].cursor < pair[1].cursor)
    );

    // Assert the durable semantic transitions carried by the broad feed, rather
    // than treating event-class presence as evidence of P3/P4 convergence.
    let transitions = events
        .iter()
        .flat_map(|event| event.invalidations.iter())
        .filter_map(|invalidation| match invalidation {
            super::events::RelInvalidation::ConversationDelta(delta) => Some(&delta.transition),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::PolicyChanged {
            policy: ConversationPolicy::VisibleAutoBranch,
            ..
        }
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::SubmissionChanged(entry)
            if entry.id.submission_id.as_str() == "admit-turn"
                && entry.state == SubmissionState::Committed
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::FinalizeWon { submission_id, branch, .. }
            if submission_id.submission_id.as_str() == "admit-turn"
                && branch.branch_id == committed_branch_id
                && branch.fork_parent_node_id.is_none()
                && branch.display_label.is_none()
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::ArchiveCommitted {
            submission_id,
            branch,
            node_id,
            ..
        } if submission_id.submission_id.as_str() == "admit-turn"
                && node_id == &committed_node_id
                && branch.branch_id == committed_branch_id
                && branch.fork_parent_node_id.is_none()
                && branch.display_label.is_none()
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::PostCommitRecorded { submission_id }
            if submission_id.submission_id.as_str() == "admit-turn"
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::GenerationChanged(observed)
            if observed.identity.operation_id == generation.operation_id
                && observed.status == super::observation::GenerationStatus::Pending
    )));
    assert!(transitions.iter().any(|transition| matches!(
        transition,
        ConversationTransition::GenerationChanged(observed)
            if observed.identity.operation_id == generation.operation_id
                && observed.status == super::observation::GenerationStatus::Running
    )));
    let broad_debug = format!(
        "{:?}{:?}",
        events,
        g.subscribe_rel(&writer, &owner, None, 8).unwrap().0
    );
    for private in ["private submitted body", "CHECKPOINT_PRIVATE_SENTINEL"] {
        assert!(
            !broad_debug.contains(private),
            "broad REL publication exposed {private}"
        );
    }
    let branches = g
        .with_rel(
            &g.authorize(&writer, &owner, OwnerGrant::ViewRel).unwrap(),
            |runtime| Ok(runtime.cva().archive().branches()),
        )
        .unwrap();
    assert!(
        branches
            .iter()
            .any(|branch| branch.conversation_id == CONVERSATION
                && branch.leaf_node_id == generation.generation_id)
    );
}

#[test]
fn late_join_gets_one_durable_prefix_and_retries_do_not_duplicate_deltas() {
    let (g, owner, writer) = setup("convergence-late-join");
    let generation = start(&g, &writer, &owner, "late-turn");
    let key = stream_key(&g, &writer, &owner, &generation);
    let (_, early) = g
        .subscribe_authoritative_conversation_stream(&writer, key.clone(), 8)
        .unwrap();
    assert!(
        g.next_conversation_stream(&writer, &early)
            .unwrap()
            .is_none()
    );

    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "prefix one",
    )
    .unwrap();
    let first = stream_event(&g, &writer, &early);
    assert!(matches!(&first.payload, StreamPayload::Checkpoint(c) if c.text == "prefix one"));

    let (late_snapshot, late) = g
        .subscribe_authoritative_conversation_stream(&writer, key, 8)
        .unwrap();
    assert_eq!(
        late_snapshot.source.checkpoint.as_ref().unwrap().text,
        "prefix one"
    );
    assert!(
        g.next_conversation_stream(&writer, &late)
            .unwrap()
            .is_none()
    );

    // P3 checkpoint retries are idempotent: they cannot enqueue a second delivery.
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "prefix one",
    )
    .unwrap();
    assert!(
        g.next_conversation_stream(&writer, &early)
            .unwrap()
            .is_none()
    );
    assert!(
        g.next_conversation_stream(&writer, &late)
            .unwrap()
            .is_none()
    );

    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        2,
        "prefix one plus two",
    )
    .unwrap();
    for sub in [&early, &late] {
        let event = stream_event(&g, &writer, sub);
        match &event.payload {
            StreamPayload::Checkpoint(content) => {
                assert_eq!(content.text, " plus two");
                assert!(matches!(
                    &content.observation.content,
                    super::observation::CheckpointContentIdentity::Delta {
                        after_checkpoint_sequence: 1,
                        ..
                    }
                ));
            }
            other => panic!("expected one delta, got {other:?}"),
        }
        assert!(g.next_conversation_stream(&writer, sub).unwrap().is_none());
    }
    g.complete_conversation_generation(&owner, CONVERSATION, &generation.operation_id)
        .unwrap();
    for sub in [&early, &late] {
        let terminal = stream_event(&g, &writer, sub);
        match &terminal.payload {
            StreamPayload::Checkpoint(content) => assert_eq!(
                content.observation.status,
                super::observation::GenerationStatus::Completed
            ),
            StreamPayload::Status(status) => assert_eq!(
                status.status,
                super::observation::GenerationStatus::Completed
            ),
        }
    }
}

#[test]
fn overflow_replaces_queued_deltas_with_a_fresh_authoritative_snapshot() {
    let (g, owner, writer) = setup("convergence-overflow");
    let generation = start(&g, &writer, &owner, "overflow-turn");
    let key = stream_key(&g, &writer, &owner, &generation);
    let (initial, stale) = g
        .subscribe_authoritative_conversation_stream(&writer, key, 1)
        .unwrap();

    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "first",
    )
    .unwrap();
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        2,
        "first second",
    )
    .unwrap();
    match g
        .next_conversation_stream(&writer, &stale)
        .unwrap()
        .unwrap()
    {
        StreamDelivery::ResyncRequired { latest_cursor, .. } => {
            assert_eq!(latest_cursor, initial.cursor + 2)
        }
        other => panic!("bounded queue must request resync, got {other:?}"),
    }

    let (fresh, replacement) = g
        .reconnect_authoritative_conversation_stream(&writer, &stale, 1)
        .unwrap();
    assert_eq!(
        fresh.source.checkpoint.as_ref().unwrap().text,
        "first second"
    );
    assert_eq!(
        fresh
            .source
            .checkpoint
            .as_ref()
            .unwrap()
            .observation
            .durable_checkpoint_sequence,
        2
    );
    assert!(
        g.next_conversation_stream(&writer, &replacement)
            .unwrap()
            .is_none()
    );
    assert!(g.next_conversation_stream(&writer, &stale).is_err());
}

#[test]
fn view_only_attachment_can_observe_but_cannot_change_shared_conversation_state() {
    let (g, owner, writer) = setup("convergence-read-only");
    let viewer = g
        .attach_instance(
            "view-only-principal".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    let generation = start(&g, &writer, &owner, "protected-turn");
    let key = stream_key(&g, &viewer, &owner, &generation);
    let (snapshot, stream) = g
        .subscribe_authoritative_conversation_stream(&viewer, key, 4)
        .unwrap();
    assert_eq!(
        snapshot.source.conversation.generations[0].status,
        super::observation::GenerationStatus::Running
    );
    assert!(
        g.next_conversation_stream(&viewer, &stream)
            .unwrap()
            .is_none()
    );
    assert!(
        g.submit_conversation(
            &viewer,
            &owner,
            request("forbidden", "attempted write", "root-one")
        )
        .is_err()
    );
    assert!(
        g.set_conversation_policy(
            &viewer,
            &owner,
            CONVERSATION,
            ConversationPolicy::SequentialChronological
        )
        .is_err()
    );
    assert!(
        g.cancel_conversation_generation(&viewer, &owner, CONVERSATION, &generation.operation_id)
            .is_err()
    );
    assert!(
        g.next_conversation_stream(&viewer, &stream)
            .unwrap()
            .is_none()
    );
}

#[test]
fn restart_recovers_interrupted_checkpoint_as_prefix_without_replaying_a_delta() {
    let state = test_path("convergence-restart-state");
    let archive = test_path("convergence-restart.rel");
    let g = gateway_at(state.clone());
    let owner = g.mount_rel(seeded(archive.clone())).unwrap();
    let writer = attach_full(&g, &owner);
    let generation = start(&g, &writer, &owner, "restart-turn");
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "recovered private prefix",
    )
    .unwrap();
    g.detach_instance(&writer).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g);

    let reopened = gateway_at(state);
    assert_eq!(
        reopened
            .mount_rel(InteractionRuntime::new(Cva::open(&archive).unwrap()))
            .unwrap(),
        owner
    );
    let writer = attach_full(&reopened, &owner);
    let status = reopened
        .conversation_generation_status(&writer, &owner, CONVERSATION, &generation.operation_id)
        .unwrap();
    let recovered = status.status.generation.unwrap();
    assert_eq!(recovered.phase, DurableGenerationPhase::Interrupted);
    assert_eq!(recovered.checkpoint.as_ref().unwrap().sequence, 1);

    let key = stream_key(&reopened, &writer, &owner, &recovered);
    let (snapshot, stream) = reopened
        .subscribe_authoritative_conversation_stream(&writer, key, 4)
        .unwrap();
    assert_eq!(
        snapshot.source.conversation.generations[0].status,
        super::observation::GenerationStatus::Interrupted
    );
    assert_eq!(
        snapshot.source.checkpoint.as_ref().unwrap().text,
        "recovered private prefix"
    );
    assert!(
        reopened
            .next_conversation_stream(&writer, &stream)
            .unwrap()
            .is_none()
    );
    let token = reopened
        .authorize(&writer, &owner, OwnerGrant::ViewRel)
        .unwrap();
    reopened
        .with_rel(&token, |runtime| {
            assert!(
                !runtime
                    .cva()
                    .archive()
                    .has_node(CONVERSATION, &generation.generation_id)
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn journal_checkpoint_is_authoritative_when_archive_stream_is_ahead() {
    let (g, owner, writer) = setup("convergence-source-cut");
    let generation = start(&g, &writer, &owner, "source-cut-turn");
    g.checkpoint_conversation_generation(&owner, CONVERSATION, &generation.operation_id, 1, "A")
        .unwrap();

    // Fault-stage a real mounted Archive stream after its sync but before the
    // P3 checkpoint receipt advances. This reproduces the crash window.
    let token = g.authorize(&writer, &owner, OwnerGrant::ViewRel).unwrap();
    g.with_rel(&token, |runtime| {
        runtime
            .cva
            .put_interaction_stream(crate::InteractionStreamRecord {
                message_id: generation.generation_id.clone(),
                session_id: CONVERSATION.into(),
                parent_message_id: Some(generation.parent_node_id.clone()),
                role: crate::InteractionRole::Agent,
                timestamp_ns: generation.started_timestamp_ns,
                content: "AB".into(),
                status: crate::InteractionStreamStatus::Streaming,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)?;
        Ok(())
    })
    .unwrap();

    let key = stream_key(&g, &writer, &owner, &generation);
    let (snapshot, stream) = g
        .subscribe_authoritative_conversation_stream(&writer, key, 4)
        .unwrap();
    let authoritative = snapshot.source.checkpoint.as_ref().unwrap();
    assert_eq!(authoritative.text, "A");
    assert_eq!(authoritative.observation.durable_checkpoint_sequence, 1);
    assert!(
        g.next_conversation_stream(&writer, &stream)
            .unwrap()
            .is_none()
    );

    // Once the journal receipt is durable, the matching new suffix is delivered
    // exactly once as a Delta, despite the archive already containing it.
    g.checkpoint_conversation_generation(&owner, CONVERSATION, &generation.operation_id, 2, "AB")
        .unwrap();
    let event = stream_event(&g, &writer, &stream);
    match &event.payload {
        StreamPayload::Checkpoint(content) => {
            assert_eq!(content.text, "B");
            assert!(matches!(
                &content.observation.content,
                super::observation::CheckpointContentIdentity::Delta {
                    after_checkpoint_sequence: 1,
                    ..
                }
            ));
        }
        other => panic!("expected authoritative suffix after receipt, got {other:?}"),
    }
    assert!(
        g.next_conversation_stream(&writer, &stream)
            .unwrap()
            .is_none()
    );
}

#[test]
fn explicit_interruption_is_a_terminal_stream_status() {
    let (g, owner, writer) = setup("convergence-interruption");
    let generation = start(&g, &writer, &owner, "interrupt-turn");
    let key = stream_key(&g, &writer, &owner, &generation);
    let (_, stream) = g
        .subscribe_authoritative_conversation_stream(&writer, key, 4)
        .unwrap();
    g.checkpoint_conversation_generation(
        &owner,
        CONVERSATION,
        &generation.operation_id,
        1,
        "partial but durable",
    )
    .unwrap();
    let checkpoint_event = stream_event(&g, &writer, &stream);
    assert!(matches!(
        &checkpoint_event.payload,
        StreamPayload::Checkpoint(_)
    ));

    let interrupted = g
        .cancel_conversation_generation(&writer, &owner, CONVERSATION, &generation.operation_id)
        .unwrap();
    assert_eq!(
        interrupted.status.generation.unwrap().phase,
        DurableGenerationPhase::Interrupted
    );
    match &stream_event(&g, &writer, &stream).payload {
        StreamPayload::Status(status) => assert_eq!(
            status.status,
            super::observation::GenerationStatus::CancellationRequested
        ),
        StreamPayload::Checkpoint(content) => assert_eq!(
            content.observation.status,
            super::observation::GenerationStatus::CancellationRequested
        ),
    }
    match &stream_event(&g, &writer, &stream).payload {
        StreamPayload::Status(status) => assert_eq!(
            status.status,
            super::observation::GenerationStatus::Interrupted
        ),
        StreamPayload::Checkpoint(content) => assert_eq!(
            content.observation.status,
            super::observation::GenerationStatus::Interrupted
        ),
    }
    assert!(
        g.next_conversation_stream(&writer, &stream)
            .unwrap()
            .is_none()
    );
}

#[test]
fn durable_archive_success_survives_distribution_cut_capacity_invalidation() {
    // The gateway refuses to fan out an unbounded cut (>128 facts), but that
    // notification failure must not turn already-synced Archive data into a
    // reported mutation failure.
    const BRANCHES_IN_ONE_CUT: usize = 129;
    let (g, owner, writer) = setup("convergence-cut-capacity");
    let broad = g.subscribe_rel(&writer, &owner, None, 256).unwrap().1;
    let token = g.authorize(&writer, &owner, OwnerGrant::ManageRel).unwrap();

    let committed = g.with_conversation_distribution(&owner, token.mount_epoch, || {
        g.with_rel(&token, |runtime| {
            for index in 0..BRANCHES_IN_ONE_CUT {
                let node_id = format!("capacity-node-{index}");
                let branch_id = format!("capacity-branch-{index}");
                runtime
                    .cva
                    .append_node(
                        node_id.clone(),
                        CONVERSATION.into(),
                        Some("root-one".into()),
                        "assistant".into(),
                        10 + index as i64,
                        &format!("durable node {index}"),
                    )
                    .map_err(operation)?;
                runtime
                    .cva
                    .append_branch(Branch {
                        id: branch_id,
                        conversation_id: CONVERSATION.into(),
                        leaf_node_id: node_id,
                        canonical: false,
                    })
                    .map_err(operation)?;
            }
            runtime.cva.sync().map_err(operation)?;
            Ok(BRANCHES_IN_ONE_CUT)
        })
    });
    assert_eq!(
        committed.unwrap(),
        BRANCHES_IN_ONE_CUT,
        "durable mutation must succeed although its notification cut is rejected"
    );

    // Invalidating the sequencer prevents stale recipients from treating an
    // incomplete publication as authoritative.
    assert!(g.next_rel_event(&writer, &broad).is_err());
    let persisted = g
        .with_rel(&token, |runtime| {
            Ok(runtime
                .cva()
                .archive()
                .branches()
                .into_iter()
                .filter(|branch| {
                    branch.conversation_id == CONVERSATION
                        && branch.id.starts_with("capacity-branch-")
                })
                .count())
        })
        .unwrap();
    assert_eq!(persisted, BRANCHES_IN_ONE_CUT);

    // Lifecycle recovery reopens with a new authoritative snapshot containing
    // the durable result, rather than replaying a partial event sequence.
    g.quiesce_rel(&owner).unwrap();
    g.reopen_rel(&owner).unwrap();
    let (fresh, _) = g.subscribe_rel(&writer, &owner, None, 8).unwrap();
    let recovered = fresh
        .state
        .branches
        .iter()
        .filter(|branch| {
            branch.conversation_id == CONVERSATION && branch.id.starts_with("capacity-branch-")
        })
        .count();
    assert_eq!(recovered, BRANCHES_IN_ONE_CUT);
}

#[test]
fn stale_visible_auto_branch_publishes_the_captured_fork_to_passive_viewers() {
    let (g, owner, manager) = setup("convergence-visible-fork");
    let concurrent_writer = g
        .attach_instance(
            "convergence-concurrent-writer".into(),
            None,
            grants(
                &owner,
                &[OwnerGrant::ViewRel, OwnerGrant::SubmitConversation],
            ),
        )
        .unwrap();
    let viewer = g
        .attach_instance(
            "convergence-passive-viewer".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    g.select_managed_conversation(&manager, &owner, CONVERSATION, "main")
        .unwrap();
    g.select_managed_conversation(&concurrent_writer, &owner, CONVERSATION, "main")
        .unwrap();
    g.select_managed_conversation(&viewer, &owner, CONVERSATION, "main")
        .unwrap();
    assert!(
        g.set_conversation_policy(
            &manager,
            &owner,
            CONVERSATION,
            ConversationPolicy::VisibleAutoBranch,
        )
        .unwrap()
    );
    let (_, passive_feed) = g.subscribe_rel(&viewer, &owner, None, 32).unwrap();

    // Both submissions capture the same valid Archive parent before either
    // finalizes. Their receive ordering chooses the main-line winner; the other
    // request must fork from the retained parent under VisibleAutoBranch.
    let first = g
        .submit_conversation(
            &manager,
            &owner,
            request("fork-candidate-a", "first candidate", "root-one"),
        )
        .unwrap();
    let journal = g.conversation.journal(&owner).unwrap();
    let current_revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new(CONVERSATION).unwrap());
    let mut second_request = request("fork-candidate-b", "second candidate", "root-one");
    second_request.expected_conversation_revision = current_revision;
    let second = g
        .submit_conversation(&concurrent_writer, &owner, second_request)
        .unwrap();

    let committed = [
        g.process_next_conversation_submission(&manager, &owner, CONVERSATION)
            .unwrap()
            .unwrap(),
        g.process_next_conversation_submission(&manager, &owner, CONVERSATION)
            .unwrap()
            .unwrap(),
    ];
    assert!(committed.iter().any(|receipt| receipt.id == first.id));
    assert!(committed.iter().any(|receipt| receipt.id == second.id));
    let main_winner = committed
        .iter()
        .find(|receipt| receipt.status.finalize_capture.as_ref().unwrap().branch_id == "main")
        .unwrap();
    let fork_winner = committed
        .iter()
        .find(|receipt| receipt.status.finalize_capture.as_ref().unwrap().branch_id != "main")
        .unwrap();
    let capture = fork_winner.status.finalize_capture.as_ref().unwrap();
    let fork_id = capture.branch_id.clone();
    assert!(fork_id.starts_with("auto-"));
    assert_eq!(capture.parent_node_id.as_deref(), Some("root-one"));
    assert_eq!(
        fork_winner
            .status
            .archive_receipt
            .as_ref()
            .unwrap()
            .branch_id,
        fork_id
    );
    assert_eq!(
        main_winner
            .status
            .archive_receipt
            .as_ref()
            .unwrap()
            .branch_id,
        "main"
    );

    let fork_submitter = if fork_winner.id == first.id {
        &manager
    } else {
        &concurrent_writer
    };
    let main_submitter = if fork_winner.id == first.id {
        &concurrent_writer
    } else {
        &manager
    };
    assert_eq!(
        g.instance_managed_state(fork_submitter, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some(fork_id.as_str())
    );
    assert_eq!(
        g.instance_managed_state(main_submitter, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some("main")
    );
    assert_eq!(
        g.instance_managed_state(&viewer, &owner)
            .unwrap()
            .branch_id
            .as_deref(),
        Some("main")
    );

    let events = broad_events(&g, &viewer, &passive_feed);
    let transitions = events
        .iter()
        .flat_map(|event| event.invalidations.iter())
        .filter_map(|invalidation| match invalidation {
            super::events::RelInvalidation::ConversationDelta(delta) => Some(&delta.transition),
            _ => None,
        })
        .collect::<Vec<_>>();
    let matching_finalize = transitions
        .iter()
        .filter_map(|transition| match transition {
            ConversationTransition::FinalizeWon {
                submission_id,
                branch,
                ..
            } if submission_id == &fork_winner.id => Some(branch),
            _ => None,
        })
        .collect::<Vec<_>>();
    let matching_branch_created = transitions
        .iter()
        .filter_map(|transition| match transition {
            ConversationTransition::BranchCreated(branch) if branch.branch_id == fork_id => {
                Some(branch)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(matching_finalize.len(), 1);
    assert_eq!(matching_finalize[0].branch_id, fork_id);
    let captured_label = capture.branch_label.as_deref().unwrap();
    assert_eq!(
        matching_finalize[0].fork_parent_node_id.as_deref(),
        Some("root-one")
    );
    assert_eq!(
        matching_finalize[0].display_label.as_deref(),
        Some(captured_label)
    );

    let matching_archive = transitions
        .iter()
        .filter_map(|transition| match transition {
            ConversationTransition::ArchiveCommitted {
                submission_id,
                branch,
                ..
            } if submission_id == &fork_winner.id => Some(branch),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(matching_archive.len(), 1);
    assert_eq!(matching_archive[0].branch_id, fork_id);
    assert_eq!(
        matching_archive[0].fork_parent_node_id.as_deref(),
        Some("root-one")
    );
    assert_eq!(
        matching_archive[0].display_label.as_deref(),
        Some(captured_label)
    );

    assert_eq!(matching_branch_created.len(), 1);
    assert_eq!(matching_branch_created[0].branch_id, fork_id);
    assert_eq!(
        matching_branch_created[0].fork_parent_node_id.as_deref(),
        Some("root-one")
    );
    assert_eq!(
        matching_branch_created[0].display_label.as_deref(),
        Some(captured_label)
    );

    let archive_branches = g
        .with_rel(
            &g.authorize(&viewer, &owner, OwnerGrant::ViewRel).unwrap(),
            |runtime| Ok(runtime.cva().archive().branches()),
        )
        .unwrap();
    assert!(
        archive_branches
            .iter()
            .any(|branch| { branch.conversation_id == CONVERSATION && branch.id == fork_id })
    );

    let fork_head = fork_winner
        .status
        .archive_receipt
        .as_ref()
        .unwrap()
        .node_id
        .clone();
    let current_revision = journal
        .lock()
        .unwrap()
        .conversation_revision(&ConversationId::new(CONVERSATION).unwrap());
    let mut continuation_request =
        request("fork-continuation", "continue fork candidate", &fork_head);
    continuation_request.expected_conversation_revision = current_revision;
    continuation_request.branch_id = fork_id.clone();
    let continuation_admission = g
        .submit_conversation(fork_submitter, &owner, continuation_request)
        .unwrap();
    let continuation = g
        .process_next_conversation_submission(fork_submitter, &owner, CONVERSATION)
        .unwrap()
        .unwrap();
    assert_eq!(continuation.id, continuation_admission.id);
    let continuation_events = broad_events(&g, &viewer, &passive_feed);
    let continuation_transitions = continuation_events
        .iter()
        .flat_map(|event| event.invalidations.iter())
        .filter_map(|invalidation| match invalidation {
            super::events::RelInvalidation::ConversationDelta(delta) => Some(&delta.transition),
            _ => None,
        })
        .collect::<Vec<_>>();
    let continuation_finalize = continuation_transitions
        .iter()
        .find_map(|transition| match transition {
            ConversationTransition::FinalizeWon {
                submission_id,
                branch,
                ..
            } if submission_id == &continuation.id => Some(branch),
            _ => None,
        })
        .expect("continuation publishes its durable branch receipt");
    let continuation_archive = continuation_transitions
        .iter()
        .find_map(|transition| match transition {
            ConversationTransition::ArchiveCommitted {
                submission_id,
                branch,
                ..
            } if submission_id == &continuation.id => Some(branch),
            _ => None,
        })
        .expect("continuation publishes its Archive receipt");
    for branch in [continuation_finalize, continuation_archive] {
        assert_eq!(branch.branch_id, fork_id);
        assert_eq!(branch.fork_parent_node_id.as_deref(), Some("root-one"));
        assert_eq!(branch.display_label.as_deref(), Some(captured_label));
    }
    assert!(
        !continuation_transitions.iter().any(|transition| matches!(
            transition,
            ConversationTransition::BranchCreated(branch) if branch.branch_id == fork_id
        )),
        "continuation on an existing fork must not publish a second BranchCreated"
    );
}

#[test]
fn checkpoint_delta_boundaries_preserve_complete_utf8_characters() {
    let (g, owner, writer) = setup("convergence-utf8-delta");
    let generation = start(&g, &writer, &owner, "utf8-turn");
    let key = stream_key(&g, &writer, &owner, &generation);
    let (_, stream) = g
        .subscribe_authoritative_conversation_stream(&writer, key, 4)
        .unwrap();

    g.checkpoint_conversation_generation(&owner, CONVERSATION, &generation.operation_id, 1, "雪")
        .unwrap();
    let first = stream_event(&g, &writer, &stream);
    match &first.payload {
        StreamPayload::Checkpoint(content) => {
            assert_eq!(content.text, "雪");
            assert!(matches!(
                &content.observation.content,
                super::observation::CheckpointContentIdentity::Delta {
                    after_checkpoint_sequence: 0,
                    utf8_bytes: 3,
                    ..
                }
            ));
        }
        other => panic!("expected first UTF-8 checkpoint delta, got {other:?}"),
    }

    g.checkpoint_conversation_generation(&owner, CONVERSATION, &generation.operation_id, 2, "雪🙂")
        .unwrap();
    let second = stream_event(&g, &writer, &stream);
    match &second.payload {
        StreamPayload::Checkpoint(content) => {
            assert_eq!(content.text, "🙂");
            assert!(matches!(
                &content.observation.content,
                super::observation::CheckpointContentIdentity::Delta {
                    after_checkpoint_sequence: 1,
                    utf8_bytes: 4,
                    ..
                }
            ));
        }
        other => panic!("expected suffix-only UTF-8 delta, got {other:?}"),
    }
    assert!(
        g.next_conversation_stream(&writer, &stream)
            .unwrap()
            .is_none()
    );
}
