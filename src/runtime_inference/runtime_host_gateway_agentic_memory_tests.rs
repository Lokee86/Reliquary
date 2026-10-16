use super::*;

fn path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-gateway-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root.join(name)
}

fn gateway() -> GatewayRuntime {
    GatewayRuntime::new(
        path("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    )
}

#[test]
fn agentic_read_envelope_is_rel_only_and_revalidates_selection_and_detachment() {
    let gateway = gateway();
    let first = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("a.rel")).unwrap(),
        ))
        .unwrap();
    let second = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("b.rel")).unwrap(),
        ))
        .unwrap();
    let phy = gateway
        .mount_phy(Phylactery::create(path("private.phy")).unwrap())
        .unwrap();
    let grants = BTreeMap::from([
        (first.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
        (second.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
        (phy.clone(), BTreeSet::from([OwnerGrant::UseOwnPhy])),
    ]);
    let instance = gateway
        .attach_instance("agentic-reader".into(), Some(phy), grants)
        .unwrap();
    gateway.select_rel(&instance, &first).unwrap();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    assert_eq!(envelope.owners.len(), 1);
    assert_eq!(envelope.owners[0].token.owner_id, first);
    assert_eq!(envelope.owners[0].cut, 0);
    assert!(envelope.owners[0].profile.is_none());
    assert!(gateway.agentic_query_embedding_route().is_err());
    assert!(gateway.agentic_decision_route().is_err());
    gateway.select_rel(&instance, &second).unwrap();
    assert!(
        gateway
            .with_agentic_read_delivery(&instance, &envelope, || Ok(()))
            .is_err()
    );
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    gateway.detach_instance(&instance).unwrap();
    assert!(
        gateway
            .with_agentic_read_delivery(&instance, &envelope, || Ok(()))
            .is_err()
    );
}

#[test]
fn agentic_read_envelope_rejects_revoked_or_regranted_authority() {
    let gateway = gateway();
    let owner = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("owner.rel")).unwrap(),
        ))
        .unwrap();
    let grants = BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]);
    let instance = gateway
        .attach_instance("reader".into(), None, grants.clone())
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    gateway
        .replace_instance_grants(&instance, BTreeMap::new())
        .unwrap();
    gateway.replace_instance_grants(&instance, grants).unwrap();
    assert!(
        gateway
            .with_agentic_read_delivery(&instance, &envelope, || Ok(()))
            .is_err()
    );
}

#[test]
fn agentic_read_envelope_uses_only_granted_dependencies_and_detects_topology_changes() {
    let gateway = gateway();
    let dependency = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("dependency.rel")).unwrap(),
        ))
        .unwrap();
    let hidden = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("hidden.rel")).unwrap(),
        ))
        .unwrap();
    let unrelated = gateway
        .mount_rel(InteractionRuntime::new(
            crate::Cva::create_project(path("unrelated.rel")).unwrap(),
        ))
        .unwrap();
    let mut root = crate::Cva::create_project(path("root.rel")).unwrap();
    root.set_rel_metadata(None, vec![dependency.clone(), hidden.clone()])
        .unwrap();
    let root = gateway.mount_rel(InteractionRuntime::new(root)).unwrap();
    let grants = [root.clone(), dependency.clone(), unrelated]
        .into_iter()
        .map(|id| (id, BTreeSet::from([OwnerGrant::ViewRel])))
        .collect();
    let instance = gateway
        .attach_instance("reader".into(), None, grants)
        .unwrap();
    gateway.select_rel(&instance, &root).unwrap();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    assert_eq!(
        envelope
            .owners
            .iter()
            .map(|owner| owner.token.owner_id.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([root.clone(), dependency.clone()])
    );
    let admin = gateway
        .attach_instance(
            "admin".into(),
            None,
            BTreeMap::from([
                (
                    root.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
                ),
                (dependency, BTreeSet::from([OwnerGrant::ViewRel])),
                (hidden, BTreeSet::from([OwnerGrant::ViewRel])),
            ]),
        )
        .unwrap();
    gateway
        .set_rel_metadata_for(&admin, &root, None, Vec::new())
        .unwrap();
    assert!(
        gateway
            .with_agentic_read_delivery(&instance, &envelope, || Ok(()))
            .is_err()
    );
}
