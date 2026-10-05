use super::super::{GatewayRuntime, InstanceRuntime, OwnerGrant};
use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};

fn setup(
    label: &str,
) -> (
    GatewayRuntime,
    String,
    Arc<InstanceRuntime>,
    OwnerAccessToken,
    Arc<RelSequencer>,
) {
    let gateway = GatewayRuntime::new(
        test_path(&format!("{label}-state")),
        super::super::ReliquaryRuntimeRoutes::default(),
        crate::InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        crate::EpisodePolicy::default(),
    );
    let owner = gateway
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path(&format!("{label}.rel"))).unwrap(),
        ))
        .unwrap();
    let instance = gateway
        .attach_instance(
            label.into(),
            None,
            BTreeMap::from([(
                owner.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        )
        .unwrap();
    let token = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let seq = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&owner)
            .unwrap()
            .sequencer,
    );
    (gateway, owner, instance, token, seq)
}

fn publish(gate: &mut RelSequencerGate<'_>, version: u64) -> Arc<RelEventEnvelope> {
    gate.publish(
        RelEventClass::ArchiveChanged,
        DurableWatermarks {
            archive_version: Some(version),
            ..Default::default()
        },
        vec![],
    )
    .unwrap()
}

#[test]
fn notification_cursor_is_monotonic_and_independent_of_owner_watermarks() {
    let (_gateway, _owner, _instance, token, seq) = setup("rel-seq-cursors");
    let mut gate = seq.gate(token.mount_epoch).unwrap();
    let id = gate.register(token.clone(), 2).unwrap();
    let first = publish(&mut gate, 70);
    let second = publish(&mut gate, 70);
    assert_eq!((first.cursor, second.cursor), (1, 2));
    assert_eq!(gate.watermarks().archive_version, Some(70));
    assert_eq!(first.watermarks.memory_version, None);
    assert_eq!(
        gate.take_next(id, &token).unwrap(),
        Some(RelDelivery::Event(first))
    );
    assert_eq!(
        gate.take_next(id, &token).unwrap(),
        Some(RelDelivery::Event(second))
    );
    assert!(gate.take_next(id, &token).unwrap().is_none());
}

#[test]
fn registrations_are_isolated_by_attachment_incarnation_and_grant_generation() {
    let (gateway, owner, instance, token, seq) = setup("rel-seq-subscribers");
    let other = gateway
        .attach_instance(
            "other-viewer".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let other_token = gateway
        .authorize(&other, &owner, OwnerGrant::ViewRel)
        .unwrap();
    let mut gate = seq.gate(token.mount_epoch).unwrap();
    let first = gate.register(token.clone(), 2).unwrap();
    let second = gate.register(other_token.clone(), 2).unwrap();
    let event = publish(&mut gate, 1);
    assert!(gate.take_next(first, &other_token).is_err());
    for change in [0, 1, 2] {
        let mut stale = token.clone();
        match change {
            0 => stale.authorization.attachment_incarnation = uuid::Uuid::new_v4(),
            1 => stale.authorization.grant_generation += 1,
            _ => stale.owner_grant_generation += 1,
        }
        assert!(gate.take_next(first, &stale).is_err());
    }
    assert_eq!(
        gate.take_next(first, &token).unwrap(),
        Some(RelDelivery::Event(Arc::clone(&event)))
    );
    assert_eq!(
        gate.take_next(second, &other_token).unwrap(),
        Some(RelDelivery::Event(event))
    );
    drop(gate);
    gateway.detach_instance(&instance).unwrap();
    assert!(
        seq.gate(token.mount_epoch)
            .unwrap()
            .take_next(first, &token)
            .is_err()
    );
}

#[test]
fn two_rels_have_independent_cursors_and_no_cross_owner_registration() {
    let (gateway, _owner, instance, token, seq) = setup("rel-seq-independent");
    let second_owner = gateway
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("rel-seq-second.rel")).unwrap(),
        ))
        .unwrap();
    let second_seq = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&second_owner)
            .unwrap()
            .sequencer,
    );
    let second_epoch = gateway
        .rel_executions
        .read()
        .unwrap()
        .get(&second_owner)
        .unwrap()
        .mount_epoch
        .load(std::sync::atomic::Ordering::SeqCst);
    let mut second_gate = second_seq.gate(second_epoch).unwrap();
    assert!(second_gate.register(token.clone(), 2).is_err());
    assert!(
        gateway
            .authorize(&instance, &second_owner, OwnerGrant::ViewRel)
            .is_err()
    );
    assert_eq!(
        publish(&mut seq.gate(token.mount_epoch).unwrap(), 5).cursor,
        1
    );
    assert_eq!(second_gate.cursor(), 0);
    assert_eq!(publish(&mut second_gate, 50).cursor, 1);
}

#[test]
fn epoch_mismatch_and_reopen_clear_old_subscription_identity() {
    let (gateway, owner, instance, token, seq) = setup("rel-seq-epoch");
    assert!(seq.gate(token.mount_epoch + 1).is_err());
    let id = seq
        .gate(token.mount_epoch)
        .unwrap()
        .register(token.clone(), 2)
        .unwrap();
    gateway.quiesce_rel_for(&instance, &owner).unwrap();
    assert!(seq.gate(token.mount_epoch).is_err());
    gateway.reopen_rel_for(&instance, &owner).unwrap();
    let fresh = gateway
        .authorize(&instance, &owner, OwnerGrant::ViewRel)
        .unwrap();
    assert!(seq.gate(token.mount_epoch).is_err());
    let mut gate = seq.gate(fresh.mount_epoch).unwrap();
    assert_eq!(gate.cursor(), 1); // The reconciled lifecycle summary starts the epoch.
    assert!(gate.take_next(id, &fresh).is_err());
    assert!(gate.register(token, 2).is_err());
}

#[test]
fn cursor_exhaustion_closes_distribution_until_gateway_reopen() {
    let (gateway, owner, _instance, token, seq) = setup("rel-seq-exhaustion");
    let id = seq
        .gate(token.mount_epoch)
        .unwrap()
        .register(token.clone(), 2)
        .unwrap();
    seq.state.lock().unwrap().cursor = u64::MAX - 1;
    let mut gate = seq.gate(token.mount_epoch).unwrap();
    assert_eq!(publish(&mut gate, 1).cursor, u64::MAX);
    assert!(
        gate.publish(RelEventClass::ArchiveChanged, Default::default(), vec![])
            .is_err()
    );
    assert_eq!(gate.cursor(), u64::MAX);
    assert!(gate.take_next(id, &token).is_err());
    assert!(
        gate.publish(RelEventClass::ArchiveChanged, Default::default(), vec![])
            .is_err()
    );
    drop(gate);
    assert!(seq.gate(token.mount_epoch).is_err());
    gateway.quiesce_rel(&owner).unwrap();
    gateway.reopen_rel(&owner).unwrap();
    let epoch = gateway
        .rel_executions
        .read()
        .unwrap()
        .get(&owner)
        .unwrap()
        .mount_epoch
        .load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(publish(&mut seq.gate(epoch).unwrap(), 2).cursor, 2);
}

#[test]
fn overflow_is_bounded_and_latched_until_a_later_snapshot_resync() {
    let (_gateway, _owner, _instance, token, seq) = setup("rel-seq-overflow");
    let mut gate = seq.gate(token.mount_epoch).unwrap();
    assert!(gate.register(token.clone(), 0).is_err());
    assert!(gate.register(token.clone(), 4097).is_err());
    let id = gate.register(token.clone(), 1).unwrap();
    publish(&mut gate, 1);
    publish(&mut gate, 2);
    publish(&mut gate, 3);
    let marker = Some(RelDelivery::ResyncRequired {
        epoch: token.mount_epoch,
        latest_cursor: 3,
    });
    assert_eq!(gate.take_next(id, &token).unwrap(), marker);
    assert_eq!(gate.take_next(id, &token).unwrap(), marker);
}

#[test]
fn grant_refresh_clears_registered_delivery_without_unmounting_owner() {
    let (gateway, owner, instance, token, seq) = setup("rel-seq-grant-refresh");
    let id = seq
        .gate(token.mount_epoch)
        .unwrap()
        .register(token.clone(), 2)
        .unwrap();
    publish(&mut seq.gate(token.mount_epoch).unwrap(), 1);
    gateway
        .replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    assert!(
        seq.gate(token.mount_epoch)
            .unwrap()
            .take_next(id, &token)
            .is_err()
    );
    assert_eq!(gateway.mounted_rel_ids().unwrap(), vec![owner]);
}

#[test]
fn unmount_and_shutdown_close_retained_sequencer_handles() {
    for shutdown in [false, true] {
        let (gateway, owner, _instance, token, seq) = setup(if shutdown {
            "rel-seq-shutdown"
        } else {
            "rel-seq-unmount"
        });
        seq.gate(token.mount_epoch)
            .unwrap()
            .register(token.clone(), 2)
            .unwrap();
        if shutdown {
            gateway.shutdown().unwrap();
            gateway.shutdown().unwrap();
        } else {
            drop(gateway.unmount_rel(&owner).unwrap());
        }
        assert!(seq.gate(token.mount_epoch).is_err());
        seq.close().unwrap();
    }
}

#[test]
fn concurrent_publishers_allocate_each_cursor_once() {
    let (_gateway, _owner, _instance, token, seq) = setup("rel-seq-concurrent");
    let workers = (0..4)
        .map(|_| {
            let seq = Arc::clone(&seq);
            std::thread::spawn(move || {
                (0..20)
                    .map(|_| publish(&mut seq.gate(token.mount_epoch).unwrap(), 1).cursor)
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let mut cursors = workers
        .into_iter()
        .flat_map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    cursors.sort_unstable();
    assert_eq!(cursors, (1..=80).collect::<Vec<_>>());
}

#[test]
fn unsupported_summary_size_fails_closed_without_claiming_cursor_continuity() {
    let (_gateway, _owner, _instance, token, seq) = setup("rel-seq-summary-bound");
    let mut gate = seq.gate(token.mount_epoch).unwrap();
    let keys = vec![RelInvalidation::ProjectCorrelation(1); 129];
    assert!(
        gate.publish(RelEventClass::ProjectStateChanged, Default::default(), keys)
            .is_err()
    );
    assert_eq!(gate.cursor(), 0);
    assert!(gate.register(token.clone(), 2).is_err());
    drop(gate);
    assert!(seq.gate(token.mount_epoch).is_err());
}
