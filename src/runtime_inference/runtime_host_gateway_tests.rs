use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, InteractionRuntime};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "runtime_host_gateway_p6_harness_tests.rs"]
mod p6_harness;

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("gateway-lifecycle-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

#[test]
fn detaching_an_instance_does_not_unmount_shared_owner() {
    let gateway = gateway();
    let cva = Cva::create_project(test_path("gateway-lifecycle.rel")).unwrap();
    let owner_id = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let grants = BTreeMap::from([(owner_id.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]);
    let first = gateway
        .attach_instance("principal-one".into(), None, grants.clone())
        .unwrap();
    let second = gateway
        .attach_instance("principal-two".into(), None, grants)
        .unwrap();
    assert!(gateway.select_rel(&first, &owner_id).unwrap());
    assert!(gateway.select_rel(&second, &owner_id).unwrap());
    assert!(gateway.detach_instance(&first).unwrap());
    assert!(
        gateway
            .authorize(&first, &owner_id, OwnerGrant::ViewRel)
            .is_err()
    );
    assert_eq!(
        gateway.active_rel_id(&second).unwrap(),
        Some(owner_id.clone())
    );
    assert_eq!(gateway.mounted_rel_ids().unwrap(), vec![owner_id.clone()]);
    gateway.detach_instance(&second).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn quiesce_and_reopen_fence_old_tokens_and_restart_workers() {
    let (finished, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let gateway = gateway();
        let cva = Cva::create_project(test_path("gateway-reopen.rel")).unwrap();
        let owner_id = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
        let grants = BTreeMap::from([(owner_id.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]);
        let instance = gateway
            .attach_instance("reopen-principal".into(), None, grants)
            .unwrap();
        let old_token = gateway
            .authorize(&instance, &owner_id, OwnerGrant::ViewRel)
            .unwrap();
        gateway.with_rel(&old_token, |_| Ok(())).unwrap();

        gateway.quiesce_rel(&owner_id).unwrap();
        assert!(gateway.with_rel(&old_token, |_| Ok(())).is_err());
        assert!(
            gateway
                .rel_executions
                .read()
                .unwrap()
                .get(&owner_id)
                .unwrap()
                .owner
                .lock()
                .unwrap()
                .workers_stopped()
        );

        gateway.reopen_rel(&owner_id).unwrap();
        let new_token = gateway
            .authorize(&instance, &owner_id, OwnerGrant::ViewRel)
            .unwrap();
        assert_ne!(old_token.mount_epoch, new_token.mount_epoch);
        assert!(gateway.with_rel(&old_token, |_| Ok(())).is_err());
        gateway.with_rel(&new_token, |_| Ok(())).unwrap();

        gateway.detach_instance(&instance).unwrap();
        drop(gateway.unmount_rel(&owner_id).unwrap());
        let _ = finished.send(());
    });
    completion
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("quiesced reopen did not complete within 30 seconds");
}

#[test]
fn grant_addition_preserves_tokens_and_revocation_fences_them() {
    let gateway = gateway();
    let cva = Cva::create_project(test_path("gateway-grant-delta.rel")).unwrap();
    let owner_id = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = gateway
        .attach_instance(
            "grant-principal".into(),
            None,
            BTreeMap::from([(
                owner_id.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]),
            )]),
        )
        .unwrap();

    let old_owner_token = gateway
        .authorize(&instance, &owner_id, OwnerGrant::SubmitConversation)
        .unwrap();
    gateway
        .replace_instance_grants(
            &instance,
            BTreeMap::from([(
                owner_id.clone(),
                BTreeSet::from([
                    OwnerGrant::ViewRel,
                    OwnerGrant::SubmitConversation,
                    OwnerGrant::ManageRel,
                ]),
            )]),
        )
        .unwrap();
    gateway.with_rel(&old_owner_token, |_| Ok(())).unwrap();
    gateway
        .authorize(&instance, &owner_id, OwnerGrant::SubmitConversation)
        .unwrap();

    gateway
        .replace_instance_grants(
            &instance,
            BTreeMap::from([(
                owner_id.clone(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
            )]),
        )
        .unwrap();
    assert!(gateway.with_rel(&old_owner_token, |_| Ok(())).is_err());
    gateway
        .authorize(&instance, &owner_id, OwnerGrant::ViewRel)
        .unwrap();
    assert!(
        gateway
            .authorize(&instance, &owner_id, OwnerGrant::SubmitConversation)
            .is_err()
    );

    gateway.detach_instance(&instance).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn shutdown_stops_owner_workers_and_is_idempotent() {
    let gateway = gateway();
    let cva = Cva::create_project(test_path("gateway-shutdown.rel")).unwrap();
    let owner_id = gateway.mount_rel(InteractionRuntime::new(cva)).unwrap();
    gateway.shutdown().unwrap();
    assert!(gateway.shutdown().is_ok());
    let rels = gateway.rel_executions.read().unwrap();
    assert!(
        rels.get(&owner_id)
            .unwrap()
            .owner
            .lock()
            .unwrap()
            .workers_stopped()
    );
    drop(rels);
    assert!(gateway.reopen_rel(&owner_id).is_err());
}

#[test]
fn short_commit_leases_drain_before_lifecycle_transitions() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    for transition in ["revoke", "detach", "quiesce", "unmount", "maintenance"] {
        let gateway = Arc::new(gateway());
        let owner = gateway
            .mount_rel(InteractionRuntime::new(
                Cva::create_project(test_path(&format!("drain-{transition}.rel"))).unwrap(),
            ))
            .unwrap();
        let instance = gateway
            .attach_instance(
                format!("drain-{transition}"),
                None,
                BTreeMap::from([(
                    owner.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
                )]),
            )
            .unwrap();
        let token = gateway
            .authorize(&instance, &owner, OwnerGrant::ManageRel)
            .unwrap();
        let gate = if transition == "revoke" || transition == "detach" {
            Arc::clone(&gateway.attachment_entry(&instance).unwrap().leases)
        } else {
            Arc::clone(
                &gateway
                    .rel_executions
                    .read()
                    .unwrap()
                    .get(&owner)
                    .unwrap()
                    .fence,
            )
        };
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let commit_gateway = Arc::clone(&gateway);
        let commit_token = token.clone();
        let commit = std::thread::spawn(move || {
            commit_gateway.with_rel(&commit_token, |runtime| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                runtime
                    .cva
                    .set_rel_metadata(Some("drained commit".into()), vec![])
                    .map_err(operation)?;
                runtime.cva.sync().map_err(operation)
            })
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let transition_gateway = Arc::clone(&gateway);
        let transition_instance = Arc::clone(&instance);
        let transition_owner = owner.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = match transition {
                "revoke" => transition_gateway
                    .replace_instance_grants(&transition_instance, BTreeMap::new()),
                "detach" => transition_gateway
                    .detach_instance(&transition_instance)
                    .map(|_| ()),
                "quiesce" => transition_gateway.quiesce_rel(&transition_owner),
                "unmount" => transition_gateway.unmount_rel(&transition_owner).map(drop),
                "maintenance" => transition_gateway
                    .quiesce_rel_for(&transition_instance, &transition_owner)
                    .and_then(|_| {
                        transition_gateway
                            .reconcile_quiesced_rel_for(&transition_instance, &transition_owner)
                    }),
                _ => unreachable!(),
            };
            done_tx.send(result).unwrap();
        });
        // Observe the actual closed gate, rather than relying on a scheduling delay.
        let deadline = Instant::now() + Duration::from_secs(5);
        while gate.snapshot().unwrap().0 != OwnerAdmission::Closed {
            if Instant::now() >= deadline {
                release_tx.send(()).unwrap();
                panic!("{transition} did not close admission");
            }
            std::thread::yield_now();
        }
        assert!(gate.snapshot_full().unwrap().2 > 0);
        assert!(matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
        assert!(
            gateway.with_rel(&token, |_| Ok(())).is_err(),
            "{transition} admitted another commit"
        );
        release_tx.send(()).unwrap();
        commit.join().unwrap().unwrap();
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert!(
            gateway.with_rel(&token, |_| Ok(())).is_err(),
            "{transition} retained stale authority"
        );
        drop(gate);
        if transition != "detach" {
            gateway.detach_instance(&instance).unwrap();
        }
        if transition != "unmount" {
            drop(gateway.unmount_rel(&owner).unwrap());
        }
    }
}

#[test]
fn failed_fence_reopen_does_not_restart_owner_workers() {
    let gateway = gateway();
    let owner = gateway
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("reopen-exhaustion.rel")).unwrap(),
        ))
        .unwrap();
    let manager = gateway
        .attach_instance(
            "reopen-exhaustion-manager".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ManageRel]))]),
        )
        .unwrap();
    gateway.quiesce_rel(&owner).unwrap();
    let fence = Arc::clone(
        &gateway
            .rel_executions
            .read()
            .unwrap()
            .get(&owner)
            .unwrap()
            .fence,
    );
    fence.state.lock().unwrap().generation = u64::MAX;
    for client_path in [false, true] {
        let result = if client_path {
            gateway.reopen_rel_for(&manager, &owner)
        } else {
            gateway.reopen_rel(&owner)
        };
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("generation is exhausted")
        );
        assert_eq!(
            gateway.owner_admission(&owner).unwrap(),
            OwnerAdmission::Closed
        );
        assert!(
            gateway
                .rel_executions
                .read()
                .unwrap()
                .get(&owner)
                .unwrap()
                .owner
                .lock()
                .unwrap()
                .workers_stopped()
        );
        assert!(
            gateway
                .authorize(&manager, &owner, OwnerGrant::ManageRel)
                .is_err()
        );
    }
    // Restore a usable generation for ordinary test cleanup, not owner dispatch.
    fence.state.lock().unwrap().generation = 1;
    gateway.detach_instance(&manager).unwrap();
    drop(fence);
    drop(gateway.unmount_rel(&owner).unwrap());
}
