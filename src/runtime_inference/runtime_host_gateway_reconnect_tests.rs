use super::*;
use crate::runtime_host_test_support::test_path;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
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
    let owner = gateway
        .mount_rel(crate::InteractionRuntime::new(
            crate::Cva::create_project(test_path(&format!("{label}.rel"))).unwrap(),
        ))
        .unwrap();
    let instance = attach(&gateway, &owner, label);
    (gateway, owner, instance)
}
fn grants(owner: &str) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(
        owner.into(),
        BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
    )])
}
fn attach(gateway: &GatewayRuntime, owner: &str, label: &str) -> Arc<InstanceRuntime> {
    gateway
        .attach_instance(label.into(), None, grants(owner))
        .unwrap()
}
fn change(gateway: &GatewayRuntime, instance: &InstanceRuntime, owner: &str, label: &str) {
    gateway
        .set_rel_metadata_for(instance, owner, Some(label.into()), vec![])
        .unwrap();
}
fn drain(gateway: &GatewayRuntime, instance: &InstanceRuntime, sub: &RelSubscription) -> Vec<u64> {
    let mut cursors = vec![];
    while let Some(delivery) = gateway.next_rel_event(instance, sub).unwrap() {
        let RelDelivery::Event(event) = delivery else {
            panic!("unexpected resync")
        };
        cursors.push(event.cursor);
    }
    cursors
}

#[test]
fn reconnect_current_and_expired_cursors_replace_one_recipient_with_fresh_snapshot() {
    let (g, owner, i) = setup("p45-reconnect");
    let (initial, mut sub) = g.subscribe_rel(&i, &owner, None, 1).unwrap();
    let (_, other) = g.subscribe_rel(&i, &owner, None, 8).unwrap();
    change(&g, &i, &owner, "one");
    change(&g, &i, &owner, "two");
    assert!(matches!(
        g.next_rel_event(&i, &sub).unwrap(),
        Some(RelDelivery::ResyncRequired { .. })
    ));
    // There is no retained window. Old same-epoch cursors recover by snapshot.
    let (snapshot, fresh) = g
        .reconnect_rel(&i, &sub, Some(initial.watermark), 2)
        .unwrap();
    assert_eq!(snapshot.watermark.cursor, 2);
    assert!(g.next_rel_event(&i, &sub).is_err());
    assert!(drain(&g, &i, &fresh).is_empty());
    assert_eq!(drain(&g, &i, &other), vec![1, 2]);
    sub = fresh;
    let (current, fresh) = g
        .reconnect_rel(&i, &sub, Some(snapshot.watermark), 2)
        .unwrap();
    assert_eq!(current, snapshot);
    change(&g, &i, &owner, "three");
    assert_eq!(drain(&g, &i, &fresh), vec![3]);
}

#[test]
fn failed_reconnect_preserves_pending_recipient_and_rejects_wrong_instance() {
    let (g, owner, i) = setup("p45-failed");
    let (snapshot, sub) = g.subscribe_rel(&i, &owner, None, 2).unwrap();
    let other = attach(&g, &owner, "other");
    change(&g, &i, &owner, "one");
    assert!(g.reconnect_rel(&other, &sub, None, 2).is_err());
    assert!(g.reconnect_rel(&i, &sub, None, 0).is_err());
    assert!(
        g.reconnect_rel(
            &i,
            &sub,
            Some(RelCursor {
                cursor: 100,
                ..snapshot.watermark
            }),
            2
        )
        .is_err()
    );
    assert_eq!(drain(&g, &i, &sub), vec![1]);
}

#[test]
fn reconnect_at_registry_limit_reuses_slot_and_failed_capacity_keeps_old_slot() {
    let (g, owner, i) = setup("p45-capacity");
    let mut subs = vec![];
    for _ in 0..1024 {
        subs.push(g.subscribe_rel(&i, &owner, None, 1).unwrap().1);
    }
    assert!(g.subscribe_rel(&i, &owner, None, 1).is_err());
    let old = subs.pop().unwrap();
    assert!(g.reconnect_rel(&i, &old, None, 4097).is_err());
    let (_, fresh) = g.reconnect_rel(&i, &old, None, 1).unwrap();
    assert!(g.next_rel_event(&i, &old).is_err());
    g.disconnect_rel(&i, &fresh).unwrap();
    assert!(g.subscribe_rel(&i, &owner, None, 1).is_ok());
}

#[test]
fn grant_refresh_revoke_and_regrant_never_revive_old_delivery_or_reconnect() {
    let (g, owner, i) = setup("p45-grants");
    let (_, sub) = g.subscribe_rel(&i, &owner, None, 2).unwrap();
    change(&g, &i, &owner, "one");
    g.replace_instance_grants(&i, grants(&owner)).unwrap();
    assert!(g.next_rel_event(&i, &sub).is_err());
    assert!(g.reconnect_rel(&i, &sub, None, 2).is_err());
    let (_, refreshed) = g.subscribe_rel(&i, &owner, None, 2).unwrap();
    g.replace_instance_grants(&i, BTreeMap::new()).unwrap();
    assert!(g.subscribe_rel(&i, &owner, None, 2).is_err());
    g.replace_instance_grants(&i, grants(&owner)).unwrap();
    assert!(g.next_rel_event(&i, &refreshed).is_err());
    assert!(g.reconnect_rel(&i, &refreshed, None, 2).is_err());
    assert!(g.subscribe_rel(&i, &owner, None, 2).is_ok());
}

#[test]
fn reopen_rotates_epoch_and_publishes_only_reconciled_safe_lifecycle_cut() {
    for explicit in [false, true] {
        let (g, owner, i) = setup(if explicit {
            "p45-reopen-explicit"
        } else {
            "p45-reopen"
        });
        let (before, sub) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
        change(&g, &i, &owner, "before-reopen");
        if explicit {
            g.quiesce_rel_for(&i, &owner).unwrap();
            g.reopen_rel_for(&i, &owner).unwrap();
        } else {
            g.quiesce_rel(&owner).unwrap();
            g.reopen_rel(&owner).unwrap();
        }
        assert!(g.next_rel_event(&i, &sub).is_err());
        assert!(
            g.reconnect_rel(&i, &sub, Some(before.watermark), 4)
                .is_err()
        );
        assert!(
            g.subscribe_rel(&i, &owner, Some(before.watermark), 4)
                .is_err()
        );
        let (after, fresh) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
        assert!(after.watermark.mount_epoch > before.watermark.mount_epoch);
        assert_eq!(after.watermark.cursor, 1); // Reconciled lifecycle summary.
        assert_eq!(after.state.lifecycle, OwnerAdmission::Open);
        assert!(drain(&g, &i, &fresh).is_empty()); // Snapshot already includes summary.
        let token = g.authorize(&i, &owner, OwnerGrant::ViewRel).unwrap();
        g.with_rel_distribution_admission(&token, |sequencer| {
            let gate = sequencer.gate(token.mount_epoch)?;
            assert_eq!(gate.watermarks(), &after.state.watermarks);
            Ok(())
        })
        .unwrap();
        change(&g, &i, &owner, "after-reopen");
        assert_eq!(drain(&g, &i, &fresh), vec![2]);
    }
}

#[test]
fn quiesced_maintenance_requires_new_epoch_snapshot_instead_of_incremental_replay() {
    let (g, owner, i) = setup("p48-maintenance");
    let (before, stale) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    assert!(g.with_quiesced_rel(&owner, |_| Ok(())).is_err());
    g.quiesce_rel(&owner).unwrap();
    assert!(g.next_rel_event(&i, &stale).is_err());
    assert!(g.subscribe_rel(&i, &owner, None, 4).is_err());

    g.with_quiesced_rel(&owner, |runtime| {
        runtime
            .cva
            .append_node(
                "maintenance-node".into(),
                "maintenance-conversation".into(),
                None,
                "user".into(),
                1,
                "durable maintenance body",
            )
            .map_err(operation)?;
        runtime
            .cva
            .append_branch(crate::Branch {
                id: "main".into(),
                conversation_id: "maintenance-conversation".into(),
                leaf_node_id: "maintenance-node".into(),
                canonical: true,
            })
            .map_err(operation)?;
        runtime.cva.sync().map_err(operation)
    })
    .unwrap();

    g.reopen_rel(&owner).unwrap();
    assert!(g.next_rel_event(&i, &stale).is_err());
    assert!(
        g.reconnect_rel(&i, &stale, Some(before.watermark), 4)
            .is_err()
    );
    assert!(
        g.subscribe_rel(&i, &owner, Some(before.watermark), 4)
            .is_err()
    );
    let (after, fresh) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    assert!(after.watermark.mount_epoch > before.watermark.mount_epoch);
    assert_eq!(after.watermark.cursor, 1);
    assert!(
        after
            .state
            .branches
            .iter()
            .any(
                |branch| branch.conversation_id == "maintenance-conversation"
                    && branch.leaf_node_id == "maintenance-node"
            )
    );
    assert!(after.state.watermarks.archive_version > before.state.watermarks.archive_version);
    assert!(!format!("{after:?}").contains("durable maintenance body"));
    assert!(drain(&g, &i, &fresh).is_empty());
}

#[test]
fn detached_queued_recipient_cannot_deliver_and_native_rel_keeps_committing() {
    let (g, owner, i) = setup("p45-detach");
    let second = attach(&g, &owner, "survivor");
    let (_, sub) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    let (_, survivor) = g.subscribe_rel(&second, &owner, None, 4).unwrap();
    change(&g, &i, &owner, "one");
    g.detach_instance(&i).unwrap();
    assert!(g.next_rel_event(&i, &sub).is_err());
    assert!(g.reconnect_rel(&i, &sub, None, 4).is_err());
    // No host adapter or generation/model endpoint is attached.
    g.reconnect_host();
    change(&g, &second, &owner, "two");
    assert_eq!(drain(&g, &second, &survivor), vec![1, 2]);
}

#[test]
fn revocation_racing_admitted_publish_prevents_all_subsequent_delivery() {
    let (g, owner, viewer) = setup("p45-revoke-race");
    let manager = attach(&g, &owner, "manager");
    let (_, sub) = g.subscribe_rel(&viewer, &owner, None, 4).unwrap();
    let token = g
        .authorize(&manager, &owner, OwnerGrant::ManageRel)
        .unwrap();
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = {
        let g = Arc::clone(&g);
        std::thread::spawn(move || {
            g.with_rel_mutation(
                &token,
                super::super::mutation_events::RelMutationFamily::Metadata,
                |runtime| {
                    held_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    runtime
                        .cva
                        .set_rel_metadata(Some("racing".into()), vec![])
                        .map_err(operation)?;
                    Ok(((), false, vec![]))
                },
            )
        })
    };
    held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let revoke = {
        let g = Arc::clone(&g);
        let viewer = Arc::clone(&viewer);
        std::thread::spawn(move || g.replace_instance_grants(&viewer, BTreeMap::new()))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while g
        .attachment_by_id(viewer.instance_id())
        .unwrap()
        .leases
        .snapshot()
        .unwrap()
        .0
        == OwnerAdmission::Open
    {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    revoke.join().unwrap().unwrap();
    assert!(g.next_rel_event(&viewer, &sub).is_err());
    assert!(g.reconnect_rel(&viewer, &sub, None, 4).is_err());
    let (snapshot, _) = g.subscribe_rel(&manager, &owner, None, 4).unwrap();
    assert_eq!(snapshot.watermark.cursor, 1);
}

#[test]
fn physical_unmount_remount_rejects_old_handles_and_cursor_without_losing_state() {
    let (g, owner, i) = setup("p45-remount");
    let (before, sub) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    change(&g, &i, &owner, "persisted");
    let cva = g.unmount_rel(&owner).unwrap();
    assert!(g.next_rel_event(&i, &sub).is_err());
    let mounted = g.mount_rel(crate::InteractionRuntime::new(cva)).unwrap();
    assert_eq!(mounted, owner);
    assert!(g.reconnect_rel(&i, &sub, None, 4).is_err());
    assert!(
        g.subscribe_rel(&i, &owner, Some(before.watermark), 4)
            .is_err()
    );
    let (snapshot, fresh) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    assert!(snapshot.watermark.mount_epoch > before.watermark.mount_epoch);
    assert!(drain(&g, &i, &fresh).is_empty());
    assert_eq!(
        g.rel_metadata_for(&i, &owner)
            .unwrap()
            .type_label
            .as_deref(),
        Some("persisted")
    );
}

#[test]
fn reconnect_reader_failure_keeps_pending_notifications_on_original_recipient() {
    let (g, owner, i) = setup("p45-reader-failure");
    let (_, sub) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    change(&g, &i, &owner, "one");
    assert!(
        g.subscribe_rel_replacing(&sub.token, None, 4, Some(sub.id), |_| Err(operation(
            "injected snapshot reader failure"
        )),)
            .is_err()
    );
    assert_eq!(drain(&g, &i, &sub), vec![1]);
}

#[test]
fn reconnect_racing_mutation_covers_old_queue_and_delivers_later_change_once() {
    let (g, owner, i) = setup("p45-reconnect-race");
    let (_, sub) = g.subscribe_rel(&i, &owner, None, 4).unwrap();
    change(&g, &i, &owner, "before");
    let old_id = sub.id;
    let token = sub.token.clone();
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let reconnect = {
        let g = Arc::clone(&g);
        std::thread::spawn(move || {
            g.subscribe_rel_replacing(&token, None, 4, Some(old_id), |_| {
                let state = g.read_rel_snapshot_state(&token)?;
                held_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(state)
            })
        })
    };
    held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let writer = {
        let g = Arc::clone(&g);
        let owner = owner.clone();
        let i = Arc::clone(&i);
        std::thread::spawn(move || change(&g, &i, &owner, "after"))
    };
    release_tx.send(()).unwrap();
    let (snapshot, fresh) = reconnect.join().unwrap().unwrap();
    writer.join().unwrap();
    assert_eq!(snapshot.watermark.cursor, 1);
    assert!(g.next_rel_event(&i, &sub).is_err());
    assert_eq!(drain(&g, &i, &fresh), vec![2]);
}
