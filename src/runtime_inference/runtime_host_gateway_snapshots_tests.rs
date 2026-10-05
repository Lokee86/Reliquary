use super::super::conversation::{GatewayConversationAttachment, GatewayConversationSubmission};
use super::*;
use crate::multiplexing_journal::{
    ConversationId, ConversationSubmissionId, Submission, SubmissionId,
};
use crate::runtime_host_test_support::test_path;
use crate::{Branch, Cva, InteractionRuntime};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Barrier, mpsc};
use std::time::{Duration, Instant};

fn setup(label: &str) -> (Arc<GatewayRuntime>, String, Arc<InstanceRuntime>) {
    let gateway = Arc::new(GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        super::super::super::ReliquaryRuntimeRoutes::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    ));
    let mut cva = Cva::create_project(test_path(&format!("{label}.rel"))).unwrap();
    for (conversation, node) in [("active", "root-active"), ("inactive", "root-inactive")] {
        cva.append_node(
            node.into(),
            conversation.into(),
            None,
            "user".into(),
            1,
            "TRANSCRIPT_BODY_SECRET",
        )
        .unwrap();
        cva.append_branch(Branch {
            id: "main".into(),
            conversation_id: conversation.into(),
            leaf_node_id: node.into(),
            canonical: true,
        })
        .unwrap();
        cva.set_conversation_title(conversation, "before".into())
            .unwrap();
        cva.set_conversation_active(conversation, conversation == "active")
            .unwrap();
    }
    cva.sync().unwrap();
    let owner = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = attach(&gateway, &owner, label);
    (gateway, owner, instance)
}

fn attach(gateway: &GatewayRuntime, owner: &str, principal: &str) -> Arc<InstanceRuntime> {
    gateway
        .attach_instance(
            principal.into(),
            None,
            BTreeMap::from([(
                owner.into(),
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::ManageRel,
                    OwnerGrant::SubmitConversation,
                ]),
            )]),
        )
        .unwrap()
}

fn title(snapshot: &RelSnapshot) -> Option<&str> {
    snapshot
        .state
        .conversations
        .iter()
        .find(|c| c.conversation_id == "inactive")
        .unwrap()
        .title
        .as_deref()
}

fn request(id: &str, revision: u64) -> GatewayConversationSubmission {
    GatewayConversationSubmission {
        conversation_id: "inactive".into(),
        submission_id: id.into(),
        expected_conversation_revision: revision,
        expected_parent_node_id: Some("root-inactive".into()),
        branch_id: "main".into(),
        content: "QUEUED_BODY_SECRET".into(),
        attachments: vec![GatewayConversationAttachment {
            attachment_id: "attachment".into(),
            filename: "PRIVATE_PATH_SECRET".into(),
            mime_type: None,
            bytes: b"ATTACHMENT_BODY_SECRET".to_vec(),
        }],
        client_timestamp_ns: None,
    }
}

/// Historical-writer fixture for validating the source reader's sequence retry
/// while its publication gate is held. Production submissions now acquire that
/// same gate, so calling the public submit API from the reader hook would recurse.
fn enqueue_legacy_journal_write(gateway: &GatewayRuntime, owner: &str, id: &str, order: i64) {
    let journal = gateway.conversation.journal(owner).unwrap();
    let conversation_id = ConversationId::new("inactive").unwrap();
    let key = ConversationSubmissionId {
        conversation_id: conversation_id.clone(),
        submission_id: SubmissionId::new(id).unwrap(),
    };
    let content = format!("legacy body {id}");
    let submission = Submission {
        id: key,
        principal_id: "legacy-snapshot-writer".into(),
        scope_fingerprint: "legacy-snapshot-scope".into(),
        payload_hash: Submission::canonical_payload_hash(&content, &[]).unwrap(),
        request_metadata: json!({
            "branch_id": "main",
            "expected_parent_node_id": "root-inactive"
        }),
        content,
        attachments: vec![],
        client_timestamp_ns: None,
    };
    let mut journal = journal.lock().unwrap();
    let revision = journal.conversation_revision(&conversation_id);
    journal.enqueue(submission, order, revision).unwrap();
}

#[test]
fn safe_authoritative_snapshot_includes_inactive_conversations_and_durable_queue() {
    let (gateway, owner, instance) = setup("rel-snapshot-safe");
    let receipt = gateway
        .submit_conversation(&instance, &owner, request("pending", 0))
        .unwrap();
    let journal = gateway.conversation.journal(&owner).unwrap();
    journal
        .lock()
        .unwrap()
        .begin_host_handoff(
            &receipt.id,
            super::super::RuntimeCapability::ConversationExecution,
            "host-operation".into(),
            [17; 32],
        )
        .unwrap();
    let (snapshot, subscription) = gateway.subscribe_rel(&instance, &owner, None, 4).unwrap();
    assert_eq!(snapshot.state.conversations.len(), 2);
    assert!(
        !snapshot
            .state
            .conversations
            .iter()
            .find(|c| c.conversation_id == "inactive")
            .unwrap()
            .active
    );
    assert_eq!(snapshot.state.branches.len(), 2);
    assert_eq!(snapshot.state.submissions.len(), 1);
    assert_eq!(snapshot.state.submissions[0].id, receipt.id);
    assert_eq!(
        snapshot.state.submissions[0].branch_id.as_deref(),
        Some("main")
    );
    assert_eq!(
        snapshot.state.submissions[0].state,
        SubmissionState::Pending
    );
    assert_eq!(
        snapshot.state.submissions[0].order.admission_sequence,
        receipt.status.admission_sequence
    );
    assert_eq!(snapshot.state.host_work.len(), 1);
    assert_eq!(snapshot.state.host_work[0].operation_id, "host-operation");
    assert_eq!(
        snapshot.state.watermarks.conversation_journal_sequence,
        Some(journal.lock().unwrap().sequence())
    );
    assert!(snapshot.state.watermarks.archive_version.unwrap() > 0);
    assert_eq!(snapshot.state.watermarks.project_correlation_sequence, None);
    let observation = snapshot
        .state
        .conversation_observation
        .as_ref()
        .expect("converged snapshot includes the authoritative P3 observation");
    assert!(observation.conversations.iter().any(|conversation| {
        conversation.conversation_id == receipt.id.conversation_id
            && conversation.submissions.iter().any(|submission| {
                submission.id == receipt.id && submission.state == SubmissionState::Pending
            })
    }));
    assert!(snapshot.watermark.cursor > 0);
    let snapshot_debug = format!("{snapshot:?}");
    for secret in [
        "TRANSCRIPT_BODY_SECRET",
        "QUEUED_BODY_SECRET",
        "ATTACHMENT_BODY_SECRET",
        "PRIVATE_PATH_SECRET",
    ] {
        assert!(!snapshot_debug.contains(secret));
    }
    assert!(!snapshot_debug.contains("payload_digest"));
    assert!(!snapshot_debug.contains(&format!("{:?}", [17u8; 32])));
    assert!(
        gateway
            .next_rel_event(&instance, &subscription)
            .unwrap()
            .is_none()
    );
}

#[test]
fn mutation_racing_subscription_is_snapshot_or_later_event_exactly_once() {
    for mutation_first in [true, false] {
        let (gateway, owner, instance) = setup(if mutation_first {
            "rel-snapshot-mutation-first"
        } else {
            "rel-snapshot-subscribe-first"
        });
        let view = gateway
            .authorize(&instance, &owner, OwnerGrant::ViewRel)
            .unwrap();
        let manage = gateway
            .authorize(&instance, &owner, OwnerGrant::ManageRel)
            .unwrap();
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let mutate_gateway = Arc::clone(&gateway);
        let subscribe_gateway = Arc::clone(&gateway);
        let mutator;
        let subscriber;
        if mutation_first {
            mutator = std::thread::spawn(move || {
                mutate_gateway.with_rel_distribution_mutation(&manage, |runtime| {
                    held_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    runtime
                        .cva
                        .set_conversation_title("inactive", "after".into())
                        .map_err(operation)?;
                    Ok(((), RelEventClass::ArchiveChanged, vec![]))
                })
            });
            held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            subscriber = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                subscribe_gateway.subscribe_rel_from_reader(&view, None, 4, |_| {
                    subscribe_gateway.read_rel_snapshot_state(&view)
                })
            });
        } else {
            subscriber = std::thread::spawn(move || {
                subscribe_gateway.subscribe_rel_from_reader(&view, None, 4, |_| {
                    let state = subscribe_gateway.read_rel_snapshot_state(&view)?;
                    held_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(state)
                })
            });
            held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            mutator = std::thread::spawn(move || {
                started_tx.send(()).unwrap();
                mutate_gateway.with_rel_distribution_mutation(&manage, |runtime| {
                    runtime
                        .cva
                        .set_conversation_title("inactive", "after".into())
                        .map_err(operation)?;
                    Ok(((), RelEventClass::ArchiveChanged, vec![]))
                })
            });
        }
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        release_tx.send(()).unwrap();
        mutator.join().unwrap().unwrap();
        let (snapshot, subscription) = subscriber.join().unwrap().unwrap();
        let delivery = gateway.next_rel_event(&instance, &subscription).unwrap();
        if mutation_first {
            assert_eq!(title(&snapshot), Some("after"));
            assert_eq!(snapshot.watermark.cursor, 1);
            assert!(delivery.is_none());
        } else {
            assert_eq!(title(&snapshot), Some("before"));
            assert_eq!(snapshot.watermark.cursor, 0);
            let Some(RelDelivery::Event(event)) = delivery else {
                panic!("missing post-snapshot event")
            };
            assert_eq!(event.cursor, 1);
            assert!(event.watermarks.archive_version > snapshot.state.watermarks.archive_version);
        }
        assert!(
            gateway
                .next_rel_event(&instance, &subscription)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn simultaneous_subscribers_have_independent_registration_and_equal_cut() {
    let (gateway, owner, first) = setup("rel-snapshot-two-subscribers");
    let second = attach(&gateway, &owner, "second");
    let start = Arc::new(Barrier::new(3));
    let workers = [Arc::clone(&first), Arc::clone(&second)]
        .into_iter()
        .map(|instance| {
            let gateway = Arc::clone(&gateway);
            let owner = owner.clone();
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                gateway.subscribe_rel(&instance, &owner, None, 2)
            })
        })
        .collect::<Vec<_>>();
    start.wait();
    let mut results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap().unwrap())
        .collect::<Vec<_>>();
    let (snapshot_two, sub_two) = results.pop().unwrap();
    let (snapshot_one, sub_one) = results.pop().unwrap();
    assert_eq!(snapshot_one, snapshot_two);
    assert_ne!(sub_one.id, sub_two.id);
    assert!(gateway.next_rel_event(&second, &sub_one).is_err());
    let manage = gateway
        .authorize(&first, &owner, OwnerGrant::ManageRel)
        .unwrap();
    gateway
        .with_rel_distribution_mutation(&manage, |runtime| {
            runtime
                .cva
                .set_conversation_title("inactive", "after".into())
                .map_err(operation)?;
            Ok(((), RelEventClass::ArchiveChanged, vec![]))
        })
        .unwrap();
    for (instance, subscription) in [(&first, &sub_one), (&second, &sub_two)] {
        let Some(RelDelivery::Event(event)) =
            gateway.next_rel_event(instance, subscription).unwrap()
        else {
            panic!("missing fanout")
        };
        assert_eq!(event.cursor, snapshot_one.watermark.cursor + 1);
    }
}

#[test]
fn grant_revocation_during_snapshot_denies_registration_and_later_delivery() {
    let (gateway, owner, instance) = setup("rel-snapshot-revocation");
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let (_, old_subscription) = gateway.subscribe_rel(&instance, &owner, None, 2).unwrap();
    let (read_tx, read_rx) = mpsc::channel();
    let snapshot_gateway = Arc::clone(&gateway);
    let snapshot = std::thread::spawn(move || {
        snapshot_gateway.subscribe_rel_from_reader(&token, None, 2, |_| {
            let state = snapshot_gateway.read_rel_snapshot_state(&token)?;
            read_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while snapshot_gateway
                .attachment_by_id(token.instance_id)?
                .leases
                .snapshot()?
                .0
                == OwnerAdmission::Open
            {
                assert!(
                    Instant::now() < deadline,
                    "revocation never closed attachment"
                );
                std::thread::yield_now();
            }
            Ok(state)
        })
    });
    read_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let revoke_gateway = Arc::clone(&gateway);
    let revoke_instance = Arc::clone(&instance);
    let revoke = std::thread::spawn(move || {
        revoke_gateway.replace_instance_grants(&revoke_instance, BTreeMap::new())
    });
    assert!(snapshot.join().unwrap().is_err());
    revoke.join().unwrap().unwrap();
    assert!(
        gateway
            .next_rel_event(&instance, &old_subscription)
            .is_err()
    );
    assert!(gateway.subscribe_rel(&instance, &owner, None, 2).is_err());
}

#[test]
fn reader_failure_and_invalid_cursor_never_register_recipients() {
    let (gateway, owner, instance) = setup("rel-snapshot-failure");
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    assert!(
        gateway
            .subscribe_rel_from_reader(&token, None, 2, |_| Err(operation(
                "injected reader failure"
            )))
            .is_err()
    );
    for cursor in [
        RelCursor {
            mount_epoch: token.mount_epoch + 1,
            cursor: 0,
        },
        RelCursor {
            mount_epoch: token.mount_epoch,
            cursor: 1,
        },
    ] {
        assert!(
            gateway
                .subscribe_rel(&instance, &owner, Some(cursor), 2)
                .is_err()
        );
    }
    assert!(gateway.subscribe_rel(&instance, &owner, None, 0).is_err());
    assert!(
        gateway
            .subscribe_rel(&instance, &owner, None, 4097)
            .is_err()
    );
    // A fresh successful snapshot consumes exactly one slot, not a phantom event.
    let (snapshot, subscription) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    assert_eq!(snapshot.watermark.cursor, 0);
    assert!(
        gateway
            .next_rel_event(&instance, &subscription)
            .unwrap()
            .is_none()
    );
}

#[test]
fn journal_changes_during_snapshot_retry_to_a_coherent_source_cut() {
    let (gateway, owner, instance) = setup("rel-snapshot-journal-retry");
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let mut reads = 0;
    let (snapshot, _) = gateway
        .subscribe_rel_from_reader(&token, None, 2, |_| {
            gateway.read_rel_snapshot_state_with_hook(&token, || {
                reads += 1;
                if reads == 1 {
                    enqueue_legacy_journal_write(&gateway, &owner, "racing-submit", 77);
                }
            })
        })
        .unwrap();
    assert_eq!(reads, 2);
    assert_eq!(snapshot.state.submissions.len(), 1);
    assert_eq!(
        snapshot.state.submissions[0].id.submission_id.as_str(),
        "racing-submit"
    );
    let sequence = gateway
        .conversation
        .journal(&owner)
        .unwrap()
        .lock()
        .unwrap()
        .sequence();
    assert_eq!(
        snapshot.state.watermarks.conversation_journal_sequence,
        Some(sequence)
    );
}

#[test]
fn repeated_journal_changes_fail_without_creating_subscription() {
    let (gateway, owner, instance) = setup("rel-snapshot-journal-unstable");
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let mut reads = 0;
    let result = gateway.subscribe_rel_from_reader(&token, None, 2, |_| {
        gateway.read_rel_snapshot_state_with_hook(&token, || {
            enqueue_legacy_journal_write(
                &gateway,
                &owner,
                &format!("racing-{reads}"),
                77 + reads as i64,
            );
            reads += 1;
        })
    });
    assert!(result.is_err());
    assert_eq!(reads, 8);
    let (snapshot, _) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    assert_eq!(snapshot.state.submissions.len(), 8);
}

#[test]
fn detach_and_remount_invalidate_snapshot_subscription_authority() {
    let (gateway, owner, instance) = setup("rel-snapshot-remount");
    let (snapshot, subscription) = gateway.subscribe_rel(&instance, &owner, None, 2).unwrap();
    gateway.quiesce_rel(&owner).unwrap();
    gateway.reopen_rel(&owner).unwrap();
    assert!(gateway.next_rel_event(&instance, &subscription).is_err());
    assert!(
        gateway
            .subscribe_rel(&instance, &owner, Some(snapshot.watermark), 2)
            .is_err()
    );
    let (_, fresh) = gateway.subscribe_rel(&instance, &owner, None, 2).unwrap();
    gateway.detach_instance(&instance).unwrap();
    assert!(gateway.next_rel_event(&instance, &fresh).is_err());
}

#[test]
fn frozen_p3_observation_is_owner_epoch_checked_without_changing_its_shape() {
    let (gateway, owner, instance) = setup("rel-snapshot-observation");
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let build = |wrong_epoch| {
        gateway.subscribe_rel_from_reader(&token, None, 2, |_| {
            let mut state = gateway.read_rel_snapshot_state(&token)?;
            state.conversation_observation = Some(RelObservationSummary {
                rel_id: RelOwnerId::parse_canonical(&owner).unwrap(),
                mount_epoch: token.mount_epoch + u64::from(wrong_epoch),
                conversations: vec![],
                durable_watermarks: state.watermarks.clone(),
            });
            Ok(state)
        })
    };
    assert!(build(true).is_err());
    let (snapshot, _) = build(false).unwrap();
    assert!(snapshot.state.conversation_observation.is_some());
}

#[test]
fn broad_snapshot_never_reads_or_exposes_an_attached_private_phy() {
    let (gateway, owner, _) = setup("rel-snapshot-private-phy");
    let mut phy = crate::Phylactery::create(test_path("rel-snapshot-private.phy")).unwrap();
    phy.publish_memory(
        None,
        0,
        crate::MemoryDraft {
            category: "knowledge".into(),
            memory_type: "fact".into(),
            authority_kind: "direct".into(),
            title: "PHY_TITLE_SECRET".into(),
            content: "PHY_BODY_SECRET".into(),
            scope: "user".into(),
            lifecycle_state: "canonical".into(),
            temporal_status: "current".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: Some(1),
            mutation_id: uuid::Uuid::new_v4().to_string(),
            created_at_ns: 1,
            updated_at_ns: 1,
        },
    )
    .unwrap();
    phy.sync().unwrap();
    let phy_id = gateway.mount_phy(phy).unwrap();
    let private_viewer = gateway
        .attach_instance(
            "private-viewer".into(),
            Some(phy_id.clone()),
            BTreeMap::from([
                (owner.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                (phy_id.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
            ]),
        )
        .unwrap();
    let (snapshot, _) = gateway
        .subscribe_rel(&private_viewer, &owner, None, 2)
        .unwrap();
    for secret in ["PHY_TITLE_SECRET", "PHY_BODY_SECRET", &phy_id] {
        assert!(!format!("{snapshot:?}").contains(secret));
    }
    assert!(
        gateway
            .subscribe_rel(&private_viewer, &phy_id, None, 2)
            .is_err()
    );
}

fn publish_title(gateway: &GatewayRuntime, instance: &InstanceRuntime, owner: &str, value: &str) {
    let token = gateway
        .authorize(instance, owner, OwnerGrant::ManageRel)
        .unwrap();
    gateway
        .with_rel_distribution_mutation(&token, |runtime| {
            runtime
                .cva
                .set_conversation_title("inactive", value.into())
                .map_err(operation)?;
            Ok(((), RelEventClass::ArchiveChanged, vec![]))
        })
        .unwrap();
}

fn assert_resync(
    gateway: &GatewayRuntime,
    instance: &InstanceRuntime,
    sub: &RelSubscription,
    cursor: u64,
) {
    assert_eq!(
        gateway.next_rel_event(instance, sub).unwrap(),
        Some(RelDelivery::ResyncRequired {
            epoch: sub.token.mount_epoch,
            latest_cursor: cursor
        })
    );
}

#[test]
fn bounded_delivery_exact_capacity_overflow_and_ack_preserve_later_events() {
    let (gateway, owner, instance) = setup("rel-resync-ack");
    let (_, sub) = gateway.subscribe_rel(&instance, &owner, None, 2).unwrap();
    publish_title(&gateway, &instance, &owner, "one");
    publish_title(&gateway, &instance, &owner, "two");
    // Exactly capacity remains ordinary delivery.
    let Some(RelDelivery::Event(event)) = gateway.next_rel_event(&instance, &sub).unwrap() else {
        panic!("capacity edge");
    };
    assert_eq!(event.cursor, 1);
    publish_title(&gateway, &instance, &owner, "three");
    publish_title(&gateway, &instance, &owner, "four");
    assert_resync(&gateway, &instance, &sub, 4);
    assert_resync(&gateway, &instance, &sub, 4); // Reading cannot clear latch.
    assert!(
        gateway
            .acknowledge_rel_resync(
                &instance,
                &sub,
                RelCursor {
                    mount_epoch: sub.token.mount_epoch,
                    cursor: 4
                }
            )
            .is_err()
    );
    let snapshot = gateway.resync_rel(&instance, &sub).unwrap();
    assert_eq!(title(&snapshot), Some("four"));
    publish_title(&gateway, &instance, &owner, "five");
    assert_resync(&gateway, &instance, &sub, 5);
    assert!(
        gateway
            .acknowledge_rel_resync(
                &instance,
                &sub,
                RelCursor {
                    mount_epoch: snapshot.watermark.mount_epoch,
                    cursor: 5
                }
            )
            .is_err()
    );
    gateway
        .acknowledge_rel_resync(&instance, &sub, snapshot.watermark)
        .unwrap();
    let Some(RelDelivery::Event(event)) = gateway.next_rel_event(&instance, &sub).unwrap() else {
        panic!("lost post-snapshot event");
    };
    assert_eq!(event.cursor, 5);
    assert!(gateway.next_rel_event(&instance, &sub).unwrap().is_none());
    assert!(
        gateway
            .acknowledge_rel_resync(&instance, &sub, snapshot.watermark)
            .is_err()
    );
}

#[test]
fn overflow_during_ack_wait_invalidates_snapshot_and_requires_new_cut() {
    let (gateway, owner, instance) = setup("rel-resync-reoverflow");
    let (_, sub) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    for value in ["one", "two", "three"] {
        publish_title(&gateway, &instance, &owner, value);
    }
    assert_resync(&gateway, &instance, &sub, 3);
    let stale = gateway.resync_rel(&instance, &sub).unwrap();
    publish_title(&gateway, &instance, &owner, "four");
    publish_title(&gateway, &instance, &owner, "five");
    assert_resync(&gateway, &instance, &sub, 5);
    assert!(
        gateway
            .acknowledge_rel_resync(&instance, &sub, stale.watermark)
            .is_err()
    );
    let fresh = gateway.resync_rel(&instance, &sub).unwrap();
    assert_eq!(title(&fresh), Some("five"));
    gateway
        .acknowledge_rel_resync(&instance, &sub, fresh.watermark)
        .unwrap();
    assert!(gateway.next_rel_event(&instance, &sub).unwrap().is_none());
    publish_title(&gateway, &instance, &owner, "six");
    let Some(RelDelivery::Event(event)) = gateway.next_rel_event(&instance, &sub).unwrap() else {
        panic!("missing resumed event");
    };
    assert_eq!(event.cursor, 6);
}

#[test]
fn disconnect_latched_recipient_and_resubscribe_preserve_other_recipient_and_work() {
    let (gateway, owner, instance) = setup("rel-resync-disconnect");
    let second = attach(&gateway, &owner, "resync-second");
    let (_, sub) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    let (_, other) = gateway.subscribe_rel(&second, &owner, None, 4).unwrap();
    publish_title(&gateway, &instance, &owner, "one");
    publish_title(&gateway, &instance, &owner, "two");
    assert_resync(&gateway, &instance, &sub, 2);
    assert!(gateway.disconnect_rel(&second, &sub).is_err());
    assert!(gateway.resync_rel(&second, &sub).is_err());
    gateway.disconnect_rel(&instance, &sub).unwrap();
    assert!(gateway.next_rel_event(&instance, &sub).is_err());
    assert!(gateway.resync_rel(&instance, &sub).is_err());
    let (snapshot, fresh) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    assert_eq!(title(&snapshot), Some("two"));
    assert_eq!(snapshot.watermark.cursor, 2);
    assert!(gateway.next_rel_event(&instance, &fresh).unwrap().is_none());
    for cursor in [1, 2] {
        let Some(RelDelivery::Event(event)) = gateway.next_rel_event(&second, &other).unwrap()
        else {
            panic!("other recipient lost");
        };
        assert_eq!(event.cursor, cursor);
    }
    publish_title(&gateway, &instance, &owner, "three");
    assert!(matches!(
        gateway.next_rel_event(&instance, &fresh).unwrap(),
        Some(RelDelivery::Event(_))
    ));
}

#[test]
fn resync_ack_revalidates_epoch_and_revoked_grant() {
    let (gateway, owner, instance) = setup("rel-resync-authority");
    let (_, sub) = gateway.subscribe_rel(&instance, &owner, None, 1).unwrap();
    publish_title(&gateway, &instance, &owner, "one");
    publish_title(&gateway, &instance, &owner, "two");
    let snapshot = gateway.resync_rel(&instance, &sub).unwrap();
    assert!(
        gateway
            .acknowledge_rel_resync(
                &instance,
                &sub,
                RelCursor {
                    mount_epoch: snapshot.watermark.mount_epoch + 1,
                    cursor: snapshot.watermark.cursor
                }
            )
            .is_err()
    );
    gateway
        .replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    assert!(
        gateway
            .acknowledge_rel_resync(&instance, &sub, snapshot.watermark)
            .is_err()
    );
    assert!(gateway.resync_rel(&instance, &sub).is_err());
}
