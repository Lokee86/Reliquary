use super::*;
use crate::runtime_host_test_support::test_path;

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("phase2-phy-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

fn attach(
    gateway: &GatewayRuntime,
    principal: &str,
    phy: &str,
) -> Result<Arc<InstanceRuntime>, ReliquaryRuntimeHostError> {
    gateway.attach_instance(
        principal.into(),
        Some(phy.into()),
        BTreeMap::from([(phy.into(), BTreeSet::from([OwnerGrant::UseOwnPhy]))]),
    )
}

fn workers(gateway: &GatewayRuntime, phy: &str) -> usize {
    gateway
        .phy_executions
        .read()
        .unwrap()
        .get(phy)
        .unwrap()
        .runtime
        .lock()
        .unwrap()
        .worker_count()
}

#[test]
fn same_principal_attachments_share_one_worker_set_and_detach_does_not_stop_it() {
    let gateway = gateway();
    let phy = gateway
        .mount_phy(Phylactery::create(test_path("p2-shared.phy")).unwrap())
        .unwrap();
    let first = attach(&gateway, "alice", &phy).unwrap();
    let runtime = Arc::clone(
        &gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&phy)
            .unwrap()
            .runtime,
    );
    let second = attach(&gateway, "alice", &phy).unwrap();
    assert_eq!(workers(&gateway, &phy), 3);
    assert!(Arc::ptr_eq(
        &runtime,
        &gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&phy)
            .unwrap()
            .runtime
    ));
    assert!(attach(&gateway, "bob", &phy).is_err());
    gateway.detach_instance(&first).unwrap();
    gateway.detach_instance(&second).unwrap();
    assert_eq!(workers(&gateway, &phy), 3);
    assert!(attach(&gateway, "bob", &phy).is_err());
    drop(runtime);
    drop(gateway.unmount_phy(&phy).unwrap());
}

#[test]
fn binding_cannot_alias_or_retarget_across_detach_and_remount() {
    let gateway = gateway();
    let first = gateway
        .mount_phy(Phylactery::create(test_path("p2-bind-a.phy")).unwrap())
        .unwrap();
    let second = gateway
        .mount_phy(Phylactery::create(test_path("p2-bind-b.phy")).unwrap())
        .unwrap();
    let alice = attach(&gateway, "alice", &first).unwrap();
    assert!(attach(&gateway, "alice", &second).is_err());
    let bob = attach(&gateway, "bob", &second).unwrap();
    assert!(
        gateway
            .authorize(&alice, &second, OwnerGrant::UseOwnPhy)
            .is_err()
    );
    assert!(
        gateway
            .authorize(&bob, &first, OwnerGrant::UseOwnPhy)
            .is_err()
    );
    gateway.detach_instance(&alice).unwrap();
    let store = gateway.unmount_phy(&first).unwrap();
    gateway.mount_phy(store).unwrap();
    assert!(attach(&gateway, "bob", &first).is_err());
    let replacement = attach(&gateway, "alice", &first).unwrap();
    gateway.detach_instance(&replacement).unwrap();
    gateway.detach_instance(&bob).unwrap();
    drop(gateway.unmount_phy(&first).unwrap());
    drop(gateway.unmount_phy(&second).unwrap());
}

#[test]
fn quiesce_reopen_and_remount_invalidate_tokens_without_stopping_other_owners() {
    let gateway = gateway();
    let first = gateway
        .mount_phy(Phylactery::create(test_path("p2-fence-a.phy")).unwrap())
        .unwrap();
    let second = gateway
        .mount_phy(Phylactery::create(test_path("p2-fence-b.phy")).unwrap())
        .unwrap();
    let rel = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(test_path("p2-fence.rel")).unwrap(),
        ))
        .unwrap();
    let alice = attach(&gateway, "alice", &first).unwrap();
    let old = gateway
        .authorize(&alice, &first, OwnerGrant::UseOwnPhy)
        .unwrap();
    gateway.with_phy(&old, |_| Ok(())).unwrap();
    gateway.quiesce_phy_for(&alice, &first).unwrap();
    assert_eq!(workers(&gateway, &first), 0);
    assert_eq!(workers(&gateway, &second), 3);
    assert_eq!(gateway.owner_admission(&rel).unwrap(), OwnerAdmission::Open);
    assert!(gateway.with_phy(&old, |_| Ok(())).is_err());
    gateway.reopen_phy_for(&alice, &first).unwrap();
    let fresh = gateway
        .authorize(&alice, &first, OwnerGrant::UseOwnPhy)
        .unwrap();
    assert_ne!(old.mount_epoch, fresh.mount_epoch);
    assert_eq!(workers(&gateway, &first), 3);
    assert!(gateway.with_phy(&old, |_| Ok(())).is_err());
    gateway.with_phy(&fresh, |_| Ok(())).unwrap();
    let store = gateway.unmount_phy(&first).unwrap();
    gateway.mount_phy(store).unwrap();
    assert!(gateway.with_phy(&fresh, |_| Ok(())).is_err());
    gateway.detach_instance(&alice).unwrap();
    drop(gateway.unmount_phy(&first).unwrap());
    drop(gateway.unmount_phy(&second).unwrap());
    drop(gateway.unmount_rel(&rel).unwrap());
}

#[test]
fn shutdown_joins_both_personal_sets_once_and_preflight_exhaustion_stays_closed() {
    let gateway = gateway();
    let first = gateway
        .mount_phy(Phylactery::create(test_path("p2-stop-a.phy")).unwrap())
        .unwrap();
    let second = gateway
        .mount_phy(Phylactery::create(test_path("p2-stop-b.phy")).unwrap())
        .unwrap();
    gateway.quiesce_phy(&first).unwrap();
    let fence = Arc::clone(
        &gateway
            .phy_executions
            .read()
            .unwrap()
            .get(&first)
            .unwrap()
            .fence,
    );
    fence.state.lock().unwrap().generation = u64::MAX;
    assert!(
        gateway
            .reopen_phy(&first)
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    assert_eq!(workers(&gateway, &first), 0);
    assert_eq!(
        gateway.owner_admission(&first).unwrap(),
        OwnerAdmission::Closed
    );
    fence.state.lock().unwrap().generation = 1;
    gateway.shutdown().unwrap();
    gateway.shutdown().unwrap();
    assert_eq!(workers(&gateway, &first), 0);
    assert_eq!(workers(&gateway, &second), 0);
    assert!(gateway.reopen_phy(&first).is_err());
}
