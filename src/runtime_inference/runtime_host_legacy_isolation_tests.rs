use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, InteractionRuntime};

fn gateway(label: &str) -> GatewayRuntime {
    GatewayRuntime::new(
        test_path(label),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}

fn routes_and_config() -> (ReliquaryRuntimeRoutes, InsomniaWorkerConfig) {
    (
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
    )
}

fn create_rel(path: &std::path::Path) {
    drop(Cva::create_project(path).unwrap());
}

fn legacy_inactive(
    path: &std::path::Path,
) -> Result<ReliquaryRuntimeHost, ReliquaryRuntimeHostError> {
    let (routes, config) = routes_and_config();
    ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(Cva::open(path).unwrap()),
        routes,
        config,
        EpisodePolicy::default(),
    )
}

fn assert_claim_conflict<T>(result: Result<T, ReliquaryRuntimeHostError>) {
    match result {
        Err(error) => assert!(error.to_string().contains("already has a live execution")),
        Ok(_) => panic!("expected the shared owner claim to reject the second execution"),
    }
}

#[test]
fn gateway_and_legacy_rel_claims_exclude_each_other_and_release_on_unmount() {
    let path = test_path("mixed-rel-gateway-first.rel");
    create_rel(&path);
    let gateway = gateway("mixed-rel-gateway-first-state");
    let owner_id = gateway.mount_rel_path(&path).unwrap();
    assert_claim_conflict(legacy_inactive(&path));
    drop(gateway.unmount_rel(&owner_id).unwrap());

    let legacy = legacy_inactive(&path).unwrap();
    let error = gateway.mount_rel_path(&path).unwrap_err();
    assert!(error.to_string().contains("already has a live execution"));
    drop(legacy.into_cva().unwrap());

    let owner_id = gateway.mount_rel_path(&path).unwrap();
    drop(gateway.unmount_rel(&owner_id).unwrap());
}

#[test]
fn mixed_phy_claims_exclude_each_other_and_release_with_legacy_host() {
    let rel_path = test_path("mixed-phy-rel.rel");
    let phy_path = test_path("mixed-phy-owner.phy");
    create_rel(&rel_path);
    drop(Phylactery::create(&phy_path).unwrap());

    let gateway = gateway("mixed-phy-gateway-first-state");
    let phy_id = gateway.mount_phy_path(&phy_path).unwrap();
    let (routes, config) = routes_and_config();
    assert_claim_conflict(ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(Cva::open(&rel_path).unwrap()),
        Phylactery::open(&phy_path).unwrap(),
        routes,
        config,
        EpisodePolicy::default(),
    ));
    // A rejected two-owner legacy start releases its provisional REL claim.
    let rel_id = gateway.mount_rel_path(&rel_path).unwrap();
    drop(gateway.unmount_rel(&rel_id).unwrap());
    drop(gateway.unmount_phy(&phy_id).unwrap());

    let (routes, config) = routes_and_config();
    let legacy = ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(Cva::open(&rel_path).unwrap()),
        Phylactery::open(&phy_path).unwrap(),
        routes,
        config,
        EpisodePolicy::default(),
    )
    .unwrap();
    let error = gateway.mount_phy_path(&phy_path).unwrap_err();
    assert!(error.to_string().contains("already has a live execution"));
    let (cva, phylactery) = legacy.into_cva_and_phylactery().unwrap();
    drop(cva);
    drop(phylactery);

    let phy_id = gateway.mount_phy_path(&phy_path).unwrap();
    drop(gateway.unmount_phy(&phy_id).unwrap());
}

#[test]
fn dynamically_attached_phy_uses_the_shared_claim_until_detach() {
    let rel_path = test_path("mixed-attach-phy-rel.rel");
    let phy_path = test_path("mixed-attach-phy-owner.phy");
    create_rel(&rel_path);
    drop(Phylactery::create(&phy_path).unwrap());

    let legacy = legacy_inactive(&rel_path).unwrap();
    let gateway = gateway("mixed-attach-phy-state");
    let phy_id = gateway.mount_phy_path(&phy_path).unwrap();
    let error = legacy
        .attach_phylactery(Phylactery::open(&phy_path).unwrap())
        .unwrap_err();
    assert!(error.to_string().contains("already has a live execution"));
    drop(gateway.unmount_phy(&phy_id).unwrap());

    legacy
        .attach_phylactery(Phylactery::open(&phy_path).unwrap())
        .unwrap();
    let error = gateway.mount_phy_path(&phy_path).unwrap_err();
    assert!(error.to_string().contains("already has a live execution"));
    drop(legacy.detach_phylactery().unwrap());
    let phy_id = gateway.mount_phy_path(&phy_path).unwrap();
    drop(gateway.unmount_phy(&phy_id).unwrap());
    drop(legacy.into_cva().unwrap());
}
