use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, Phylactery};

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("gateway-admission-state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}
#[test]
fn hard_link_alias_reuses_execution_and_conflicting_copy_is_rejected_before_recovery() {
    let path = test_path("mount-admission.rel");
    let cva = Cva::create_project(&path).unwrap();
    let id = cva.owner_id().unwrap();
    drop(cva);
    let alias = test_path("mount-alias.rel");
    std::fs::hard_link(&path, &alias).unwrap();
    let copy = test_path("mount-conflicting-copy.rel");
    std::fs::copy(&path, &copy).unwrap();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&copy)
        .unwrap()
        .write_all(&[1, 2, 3])
        .unwrap();
    let before = std::fs::read(&copy).unwrap();
    let gateway = gateway();
    assert_eq!(gateway.mount_rel_path(&path).unwrap(), id);
    let (owner, epoch) = {
        let rels = gateway.rel_executions.read().unwrap();
        let rel = rels.get(&id).unwrap();
        (
            Arc::clone(&rel.owner),
            rel.mount_epoch.load(Ordering::SeqCst),
        )
    };
    assert!(!gateway.owner_mount_is_path_scoped(&id).unwrap());
    assert_eq!(gateway.mount_rel_path(&alias).unwrap(), id);
    assert_eq!(
        gateway
            .mount_rel_path(
                path.parent()
                    .unwrap()
                    .join(".")
                    .join(path.file_name().unwrap())
            )
            .unwrap(),
        id
    );
    let rels = gateway.rel_executions.read().unwrap();
    assert_eq!(rels.len(), 1);
    assert!(Arc::ptr_eq(&owner, &rels.get(&id).unwrap().owner));
    assert_eq!(
        epoch,
        rels.get(&id).unwrap().mount_epoch.load(Ordering::SeqCst)
    );
    drop(rels);
    assert!(gateway.mount_rel_path(&copy).is_err());
    // Cva::open would truncate the incomplete tail; rejected copies must stay untouched.
    assert_eq!(std::fs::read(&copy).unwrap(), before);
    drop(owner);
    drop(gateway.unmount_rel(&id).unwrap());
}
#[test]
fn phy_alias_reuses_execution_and_copy_or_wrong_kind_fails() {
    let path = test_path("mount-admission.phy");
    let phy = Phylactery::create(&path).unwrap();
    let id = phy.owner_id().unwrap();
    drop(phy);
    let alias = test_path("mount-alias.phy");
    let copy = test_path("mount-conflicting-copy.phy");
    std::fs::hard_link(&path, &alias).unwrap();
    std::fs::copy(&path, &copy).unwrap();
    let gateway = gateway();
    assert!(gateway.mount_rel_path(&path).is_err());
    assert_eq!(gateway.mount_phy_path(&path).unwrap(), id);
    assert_eq!(gateway.mount_phy_path(&alias).unwrap(), id);
    assert!(gateway.mount_phy_path(&copy).is_err());
    assert_eq!(gateway.mounted_phy_ids().unwrap(), vec![id.clone()]);
    drop(gateway.unmount_phy(&id).unwrap());
}
#[test]
fn invalid_header_and_noncanonical_owner_ids_fail_without_journal_creation() {
    let path = test_path("mount-invalid.rel");
    std::fs::write(&path, b"invalid header").unwrap();
    let gateway = gateway();
    assert!(gateway.mount_rel_path(&path).is_err());
    let nil_path = test_path("mount-nil-owner.rel");
    drop(Cva::create_project(&nil_path).unwrap());
    let mut nil_header = std::fs::read(&nil_path).unwrap();
    // The typed owner header stores its UUID after the 16-byte header and 8-byte identity.
    nil_header[24..40].fill(0);
    nil_header.extend_from_slice(&[1, 2, 3]);
    std::fs::write(&nil_path, &nil_header).unwrap();
    assert!(gateway.mount_rel_path(&nil_path).is_err());
    assert_eq!(std::fs::read(&nil_path).unwrap(), nil_header);
    assert!(validate_owner_id("rel-00000000-0000-0000-0000-000000000000", "rel-").is_err());
    assert!(validate_owner_id("rel-AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA", "rel-").is_err());
    assert!(validate_owner_id("phy-11111111-1111-4111-8111-111111111111", "rel-").is_err());
    let uuid = uuid::Uuid::new_v4();
    for prefix in REL_OWNER_PREFIXES {
        assert!(validate_rel_owner_id(&format!("{prefix}{uuid}")).is_ok());
    }
    assert!(validate_rel_owner_id(&format!("phy-{uuid}")).is_err());
    assert!(gateway.mounted_rel_ids().unwrap().is_empty());
    assert!(!gateway.state_dir().join("conversation-queues").exists());
}
#[test]
fn separate_gateway_cannot_construct_second_owner_and_unmount_releases_claim() {
    let path = test_path("mount-process-claim.rel");
    let cva = Cva::create_project(&path).unwrap();
    let id = cva.owner_id().unwrap();
    drop(cva);
    let first = gateway();
    let second = gateway();
    first.mount_rel_path(&path).unwrap();
    assert!(second.mount_rel_path(&path).is_err());
    assert!(second.mounted_rel_ids().unwrap().is_empty());
    assert!(!second.state_dir().join("conversation-queues").exists());
    drop(first.unmount_rel(&id).unwrap());
    assert_eq!(second.mount_rel_path(&path).unwrap(), id);
    drop(second.unmount_rel(&id).unwrap());
}
struct Admission {
    rel: String,
    phy: String,
}
impl GatewayAdmissionAdapter for Admission {
    fn verify(&self, credential: &str) -> Result<VerifiedInstanceAdmission, String> {
        let (principal, phy_id, grants) = match credential {
            "valid" => (
                "verified-principal",
                Some(self.phy.clone()),
                BTreeMap::from([
                    (self.rel.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                    (self.phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
                ]),
            ),
            "other-principal" => (
                "someone-else",
                Some(self.phy.clone()),
                BTreeMap::from([(self.phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy]))]),
            ),
            "rel-with-phy-grant" => (
                "verified-principal",
                None,
                BTreeMap::from([(self.rel.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy]))]),
            ),
            "phy-with-rel-grant" => (
                "verified-principal",
                Some(self.phy.clone()),
                BTreeMap::from([(
                    self.phy.clone(),
                    BTreeSet::from([OwnerGrant::UseOwnPhy, OwnerGrant::ViewRel]),
                )]),
            ),
            "unmounted-phy" => (
                "verified-principal",
                Some(format!("phy-{}", Uuid::new_v4())),
                BTreeMap::new(),
            ),
            _ => return Err("forged or expired".into()),
        };
        Ok(VerifiedInstanceAdmission {
            principal_id: principal.into(),
            phy_id,
            grants,
        })
    }
}
#[test]
fn authentication_resolves_principal_phy_and_grants_and_rejects_credential_substitution() {
    let gateway = gateway();
    assert!(gateway.attach_authenticated_instance("forged").is_err());
    let path = test_path("admission-verified.rel");
    drop(Cva::create_project(&path).unwrap());
    let rel = gateway.mount_rel_path(&path).unwrap();
    let path = test_path("admission-verified.phy");
    drop(Phylactery::create(&path).unwrap());
    let phy = gateway.mount_phy_path(&path).unwrap();
    let gateway = gateway.with_admission_adapter(Arc::new(Admission {
        rel: rel.clone(),
        phy: phy.clone(),
    }));
    for credential in [
        "forged",
        "rel-with-phy-grant",
        "phy-with-rel-grant",
        "unmounted-phy",
    ] {
        assert!(
            gateway.attach_authenticated_instance(credential).is_err(),
            "{credential}"
        );
    }
    let first = gateway.attach_authenticated_instance("valid").unwrap();
    let second = gateway.attach_authenticated_instance("valid").unwrap();
    assert_eq!(first.principal_id(), "verified-principal");
    assert_eq!(first.attached_phy_owner_id(), Some(phy.as_str()));
    assert_ne!(first.instance_id(), second.instance_id());
    assert_eq!(gateway.mounted_rel_ids().unwrap(), vec![rel.clone()]);
    gateway.select_rel(&first, &rel).unwrap();
    gateway.select_rel(&second, &rel).unwrap();
    assert!(
        gateway
            .authorize(&first, &phy, OwnerGrant::UseOwnPhy)
            .is_ok()
    );
    assert!(
        gateway
            .refresh_authenticated_instance(&first, "other-principal")
            .is_err()
    );
    assert!(
        gateway
            .refresh_authenticated_instance(&first, "rel-with-phy-grant")
            .is_err()
    );
    gateway
        .refresh_authenticated_instance(&first, "valid")
        .unwrap();
    gateway.detach_instance(&first).unwrap();
    assert_eq!(gateway.active_rel_id(&second).unwrap(), Some(rel.clone()));
    assert!(
        gateway
            .refresh_authenticated_instance(&first, "valid")
            .is_err()
    );
    gateway.detach_instance(&second).unwrap();
    drop(gateway.unmount_phy(&phy).unwrap());
    drop(gateway.unmount_rel(&rel).unwrap());
}

#[test]
fn legacy_typed_rel_mounts_reuse_uuid_journal_and_distribution_identity() {
    use crate::{Cva, ReliquaryScopeKind};
    for scope in [
        None,
        Some(ReliquaryScopeKind::Project),
        Some(ReliquaryScopeKind::Organization),
        Some(ReliquaryScopeKind::Connection),
    ] {
        let path = test_path("legacy-typed-native.rel");
        let uuid = uuid::Uuid::new_v4();
        let rel = match scope {
            Some(scope) => Cva::create_legacy_scope_with_uuid(&path, scope, *uuid.as_bytes()),
            None => Cva::create_rel_with_uuid(&path, *uuid.as_bytes(), None),
        }
        .unwrap();
        let owner = rel.owner_id().unwrap();
        drop(rel);
        let bytes = std::fs::read(&path).unwrap();
        let gateway = gateway();
        assert_eq!(gateway.mount_rel_path(&path).unwrap(), owner);
        assert_eq!(
            gateway
                .conversation
                .journal_for_test(&owner)
                .unwrap()
                .lock()
                .unwrap()
                .rel_id()
                .as_uuid(),
            uuid
        );
        let instance = gateway
            .attach_instance(
                "legacy-owner-reader".into(),
                None,
                BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
            )
            .unwrap();
        gateway.select_rel(&instance, &owner).unwrap();
        let (snapshot, subscription) = gateway.subscribe_rel(&instance, &owner, None, 8).unwrap();
        assert_eq!(snapshot.rel_id.as_uuid(), uuid);
        let (reconnected, _replacement) = gateway
            .reconnect_rel(&instance, &subscription, None, 8)
            .unwrap();
        assert_eq!(reconnected.rel_id.as_uuid(), uuid);
        let canonical = format!("rel-{uuid}");
        if owner != canonical {
            assert!(
                gateway
                    .authorize(&instance, &canonical, OwnerGrant::ViewRel)
                    .is_err(),
                "typed identity normalization cannot alias grants"
            );
            assert!(
                crate::multiplexing_journal::RelOwnerId::parse_canonical(&owner).is_err(),
                "public canonical parser stays strict"
            );
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        gateway.detach_instance(&instance).unwrap();
        drop(gateway.unmount_rel(&owner).unwrap());
    }
}
