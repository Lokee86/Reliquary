//! Production P4.7 gate/source race coverage using real mounted Archive and gateway readers.
use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Branch, Cva, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

fn setup(label: &str) -> (Arc<GatewayRuntime>, String, Arc<InstanceRuntime>) {
    let gateway = Arc::new(GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        crate::ReliquaryRuntimeRoutes::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    ));
    let mut cva = Cva::create_project(test_path(&format!("{label}.rel"))).unwrap();
    cva.append_node(
        "race-root".into(),
        "race-conversation".into(),
        None,
        "user".into(),
        1,
        "initial body",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "race-conversation".into(),
        leaf_node_id: "race-root".into(),
        canonical: true,
    })
    .unwrap();
    cva.sync().unwrap();
    let owner = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = gateway
        .attach_instance(
            label.into(),
            None,
            BTreeMap::from([(
                owner.clone(),
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::ManageRel,
                    OwnerGrant::SubmitConversation,
                ]),
            )]),
        )
        .unwrap();
    (gateway, owner, instance)
}

#[test]
fn broad_subscribe_racing_archive_commit_captures_new_branch_head_once() {
    use super::observation::ConversationTransition;
    use super::subscriptions::RelDelivery;

    let (gateway, owner, viewer) = setup("p47-broad-source-race");
    let (initial_snapshot, existing_subscription) =
        gateway.subscribe_rel(&viewer, &owner, None, 8).unwrap();
    assert_eq!(initial_snapshot.watermark.cursor, 0);

    let manage = gateway
        .authorize(&viewer, &owner, OwnerGrant::ManageRel)
        .unwrap();
    let epoch = manage.mount_epoch;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (mutation_done_tx, mutation_done_rx) = mpsc::channel();

    let mutation_gateway = Arc::clone(&gateway);
    let mutation_owner = owner.clone();
    let mutation = std::thread::spawn(move || {
        let result =
            mutation_gateway.with_conversation_distribution(&mutation_owner, epoch, || {
                mutation_gateway.with_rel(&manage, |runtime| {
                    runtime
                        .cva
                        .append_node(
                            "race-head".into(),
                            "race-conversation".into(),
                            Some("race-root".into()),
                            "assistant".into(),
                            2,
                            "committed branch body",
                        )
                        .map_err(operation)?;
                    runtime
                        .cva
                        .append_branch(Branch {
                            id: "main".into(),
                            conversation_id: "race-conversation".into(),
                            leaf_node_id: "race-head".into(),
                            canonical: true,
                        })
                        .map_err(operation)?;
                    runtime.cva.sync().map_err(operation)?;
                    Ok(())
                })?;
                // The durable Archive change is complete, but the P4 gate is still
                // held and its after-source/publication cut has not run.
                entered_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(operation)?;
                Ok(())
            });
        mutation_done_tx.send(result).unwrap();
    });

    if let Err(error) = entered_rx.recv_timeout(Duration::from_secs(5)) {
        // The closure itself also has a bounded release wait, so a broken
        // mutation path cannot strand its worker while the test reports it.
        let result = mutation_done_rx.recv_timeout(Duration::from_secs(6));
        mutation.join().expect("mutation thread panicked");
        panic!("Archive mutation did not reach its held-gate point: {error}; result: {result:?}");
    }
    let (started_tx, started_rx) = mpsc::channel();
    let subscriber_gateway = Arc::clone(&gateway);
    let subscriber_viewer = Arc::clone(&viewer);
    let subscriber_owner = owner.clone();
    let (done_tx, done_rx) = mpsc::channel();
    let subscriber = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx
            .send(subscriber_gateway.subscribe_rel(&subscriber_viewer, &subscriber_owner, None, 8))
            .unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(
            done_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "new subscriber must wait behind the held P4 publication gate"
    );

    release_tx.send(()).unwrap();
    mutation_done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("mutation/distribution cut deadlocked behind its gate")
        .expect("durable mutation/distribution cut failed");
    mutation.join().expect("mutation thread panicked");
    let (snapshot, subscription) = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("subscription deadlocked behind the P4 gate")
        .expect("authoritative broad subscription failed");
    subscriber.join().expect("subscriber thread panicked");

    let observed = snapshot
        .state
        .conversation_observation
        .as_ref()
        .expect("fresh snapshot omitted the P3 conversation source");
    let conversation = observed
        .conversations
        .iter()
        .find(|conversation| conversation.conversation_id.as_str() == "race-conversation")
        .unwrap();
    assert_eq!(
        conversation
            .branches
            .iter()
            .find(|branch| branch.branch_id == "main")
            .and_then(|branch| branch.head_node_id.as_deref()),
        Some("race-head"),
        "the branch committed while the gate was held must appear in the fresh snapshot"
    );
    assert!(snapshot.watermark.cursor >= 1);
    assert!(
        gateway
            .next_rel_event(&viewer, &subscription)
            .unwrap()
            .is_none(),
        "the already-included branch must not arrive again after the snapshot"
    );
    assert!(!format!("{snapshot:?}").contains("committed branch body"));

    let Some(RelDelivery::Event(event)) = gateway
        .next_rel_event(&viewer, &existing_subscription)
        .unwrap()
    else {
        panic!("existing broad subscriber missed the ArchiveChanged event");
    };
    assert_eq!(event.class, super::events::RelEventClass::ArchiveChanged);
    assert!(
        !event.invalidations.iter().any(|invalidation| matches!(
            invalidation,
            super::events::RelInvalidation::ConversationDelta(delta)
                if matches!(
                    &delta.transition,
                    ConversationTransition::BranchCreated(_)
                        | ConversationTransition::ArchiveCommitted { .. }
                )
        )),
        "a raw Archive sync must not invent a typed branch-creation or receipt fact"
    );
    assert!(
        gateway
            .next_rel_event(&viewer, &existing_subscription)
            .unwrap()
            .is_none()
    );

    gateway.detach_instance(&viewer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}

#[test]
fn exact_stream_join_racing_checkpoint_commit_gets_prefix_or_later_delta() {
    use super::conversation::GatewayConversationSubmission;
    use super::streams::{StreamDelivery, StreamKey, StreamPayload};
    use crate::multiplexing_journal::{ConversationId, RelOwnerId};

    let (gateway, owner, viewer) = setup("p47-checkpoint-source-race");
    gateway
        .submit_conversation(
            &viewer,
            &owner,
            GatewayConversationSubmission {
                conversation_id: "race-conversation".into(),
                submission_id: "generation-input".into(),
                expected_conversation_revision: 0,
                expected_parent_node_id: Some("race-root".into()),
                branch_id: "main".into(),
                content: "input".into(),
                attachments: Vec::new(),
                client_timestamp_ns: None,
            },
        )
        .unwrap();
    gateway
        .process_next_accepted_conversation_submission(&owner, "race-conversation")
        .unwrap()
        .unwrap();
    let generation = gateway
        .start_conversation_generation(&owner, "race-conversation", "generation-input")
        .unwrap()
        .status
        .generation
        .unwrap();
    gateway
        .checkpoint_conversation_generation(
            &owner,
            "race-conversation",
            &generation.operation_id,
            1,
            "prefix A",
        )
        .unwrap();

    let token = gateway
        .authorize(&viewer, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let key = StreamKey {
        rel_id: RelOwnerId::parse_canonical(&owner).unwrap(),
        mount_epoch: token.mount_epoch,
        conversation_id: ConversationId::new("race-conversation").unwrap(),
        operation_id: generation.operation_id.clone(),
        generation_id: generation.generation_id.clone(),
        branch_id: generation.branch_id.clone(),
    };
    let start = Arc::new(Barrier::new(3));
    let subscriber_gateway = Arc::clone(&gateway);
    let subscriber_viewer = Arc::clone(&viewer);
    let subscriber_key = key.clone();
    let subscriber_start = Arc::clone(&start);
    let (snapshot_tx, snapshot_rx) = mpsc::channel();
    let subscriber = std::thread::spawn(move || {
        subscriber_start.wait();
        snapshot_tx
            .send(
                subscriber_gateway.subscribe_authoritative_conversation_stream(
                    &subscriber_viewer,
                    subscriber_key,
                    8,
                ),
            )
            .unwrap();
    });

    let writer_gateway = Arc::clone(&gateway);
    let writer_owner = owner.clone();
    let writer_generation = generation.clone();
    let writer_start = Arc::clone(&start);
    let (checkpoint_tx, checkpoint_rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        writer_start.wait();
        checkpoint_tx
            .send(writer_gateway.checkpoint_conversation_generation(
                &writer_owner,
                "race-conversation",
                &writer_generation.operation_id,
                2,
                "prefix AB",
            ))
            .unwrap();
    });
    start.wait();

    let (snapshot, stream) = snapshot_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("exact stream join deadlocked behind checkpoint publication")
        .expect("authoritative stream snapshot failed");
    checkpoint_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("checkpoint publication deadlocked behind stream join")
        .expect("durable checkpoint failed");
    subscriber
        .join()
        .expect("stream subscriber thread panicked");
    writer.join().expect("checkpoint writer thread panicked");

    let snapshot_generation = snapshot
        .source
        .conversation
        .generations
        .iter()
        .find(|g| g.identity.generation_id == generation.generation_id)
        .unwrap();
    let checkpoint = snapshot
        .source
        .checkpoint
        .as_ref()
        .expect("running generation should expose its durable prefix");
    let mut reconstructed = checkpoint.text.clone();
    match gateway.next_conversation_stream(&viewer, &stream).unwrap() {
        None => {
            assert_eq!(snapshot_generation.durable_checkpoint_sequence, Some(2));
            assert_eq!(reconstructed, "prefix AB");
        }
        Some(StreamDelivery::Event(event)) => {
            assert_eq!(snapshot_generation.durable_checkpoint_sequence, Some(1));
            match &event.payload {
                StreamPayload::Checkpoint(delta) => {
                    assert_eq!(delta.observation.durable_checkpoint_sequence, 2);
                    assert!(matches!(
                        &delta.observation.content,
                        super::observation::CheckpointContentIdentity::Delta {
                            after_checkpoint_sequence: 1,
                            ..
                        }
                    ));
                    reconstructed.push_str(&delta.text);
                }
                other => panic!("expected durable checkpoint delta, got {other:?}"),
            }
            assert_eq!(reconstructed, "prefix AB");
            assert!(
                gateway
                    .next_conversation_stream(&viewer, &stream)
                    .unwrap()
                    .is_none()
            );
        }
        Some(other) => panic!("unexpected stream delivery: {other:?}"),
    }

    gateway
        .disconnect_conversation_stream(&viewer, &stream)
        .unwrap();
    gateway.detach_instance(&viewer).unwrap();
    drop(gateway.unmount_rel(&owner).unwrap());
}
