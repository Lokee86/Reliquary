use super::*;

#[derive(Clone, Copy, Debug)]
enum ManageOp {
    Create,
    CreateAmbient,
    Replace,
    RenameCommunity,
    Relation,
    Metadata,
    MetadataAmbient,
    Processing,
}
impl ManageOp {
    fn explicit(self) -> bool {
        !matches!(self, Self::CreateAmbient | Self::MetadataAmbient)
    }
    fn call(
        self,
        g: &GatewayRuntime,
        i: &InstanceRuntime,
        owner: &str,
        a: crate::MemoryId,
        b: crate::MemoryId,
        revision: u64,
        community: crate::CommunityId,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        match self {
            Self::Create => g
                .create_knowledge_memory_for(
                    i,
                    owner,
                    "created".into(),
                    "content".into(),
                    "knowledge".into(),
                    "fact".into(),
                    2,
                )
                .map(|_| ()),
            Self::CreateAmbient => g
                .create_knowledge_memory(
                    i,
                    "ambient".into(),
                    "content".into(),
                    "knowledge".into(),
                    "fact".into(),
                    2,
                )
                .map(|_| ()),
            Self::Replace => g
                .replace_knowledge_memory_for(
                    i,
                    owner,
                    a,
                    revision,
                    "bluejay".into(),
                    "searchable bluejay evidence".into(),
                    2,
                )
                .map(|_| ()),
            Self::RenameCommunity => {
                g.rename_knowledge_community_for(i, owner, community, "renamed".into())
            }
            Self::Relation => g.mutate_knowledge_relation_for(
                i,
                owner,
                a,
                b,
                None,
                Some(crate::GraphRelationKind::Factual),
            ),
            Self::Metadata => g
                .set_rel_metadata_for(i, owner, Some("managed".into()), vec![])
                .map(|_| ()),
            Self::MetadataAmbient => g
                .set_rel_metadata(i, Some("ambient managed".into()), vec![])
                .map(|_| ()),
            Self::Processing => g.set_rel_processing_enabled_for(i, owner, false),
        }
    }
}
const MANAGEMENT: &[ManageOp] = &[
    ManageOp::Create,
    ManageOp::CreateAmbient,
    ManageOp::Replace,
    ManageOp::RenameCommunity,
    ManageOp::Relation,
    ManageOp::Metadata,
    ManageOp::MetadataAmbient,
    ManageOp::Processing,
];

#[test]
fn management_families_require_manage_rel_on_the_exact_attached_owner() {
    let g = gateway(ReliquaryRuntimeRoutes::default());
    let (cva, a, b, community, _) = seeded();
    let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let other = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("ungranted-management.rel")).unwrap(),
        ))
        .unwrap();
    let manager = g
        .attach_instance(
            "manager".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
        )
        .unwrap();
    let viewer = g
        .attach_instance(
            "wrong-management-grant".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageProject]),
        )
        .unwrap();
    g.select_rel(&manager, &owner).unwrap();
    g.select_rel(&viewer, &owner).unwrap();
    for op in MANAGEMENT {
        let revision = g
            .knowledge_memory_for(&manager, &owner, a)
            .unwrap()
            .revision;
        let error = op
            .call(&g, &viewer, &owner, a, b, revision, community)
            .unwrap_err()
            .to_string();
        assert!(error.contains("ManageRel"), "{op:?}: {error}");
        if op.explicit() {
            let error = op
                .call(&g, &manager, &other, a, b, revision, community)
                .unwrap_err()
                .to_string();
            assert!(error.contains("ManageRel"), "{op:?}: {error}");
        }
        op.call(&g, &manager, &owner, a, b, revision, community)
            .unwrap_or_else(|error| panic!("{op:?}: authorized operation failed: {error}"));
    }
    g.replace_instance_grants(&manager, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    for op in MANAGEMENT {
        assert!(
            op.call(&g, &manager, &owner, a, b, 0, community).is_err(),
            "{op:?}: revoked"
        );
    }
    g.replace_instance_grants(
        &manager,
        grants(&owner, &[OwnerGrant::ViewRel, OwnerGrant::ManageRel]),
    )
    .unwrap();
    g.detach_instance(&manager).unwrap();
    for op in MANAGEMENT {
        assert!(
            op.call(&g, &manager, &owner, a, b, 0, community).is_err(),
            "{op:?}: detached"
        );
    }
    g.detach_instance(&viewer).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g.unmount_rel(&other).unwrap());
}

#[test]
fn instance_maintenance_requires_current_manage_rel_authority() {
    let g = gateway(ReliquaryRuntimeRoutes::default());
    let owner = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("managed-maintenance.rel")).unwrap(),
        ))
        .unwrap();
    let other = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("foreign-maintenance.rel")).unwrap(),
        ))
        .unwrap();
    let manager = g
        .attach_instance(
            "maintenance-manager".into(),
            None,
            grants(&owner, &[OwnerGrant::ManageRel]),
        )
        .unwrap();
    let viewer = g
        .attach_instance(
            "maintenance-viewer".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    assert!(
        g.set_rel_processing_enabled_for(&viewer, &owner, false)
            .is_err()
    );
    assert!(g.quiesce_rel_for(&viewer, &owner).is_err());
    assert!(g.quiesce_rel_for(&manager, &other).is_err());
    g.quiesce_rel_for(&manager, &owner).unwrap();
    g.quiesce_rel(&other).unwrap();
    assert!(g.reconcile_quiesced_rel_for(&viewer, &owner).is_err());
    assert!(g.reconcile_quiesced_rel_for(&manager, &other).is_err());
    g.reconcile_quiesced_rel_for(&manager, &owner).unwrap();
    assert!(g.reopen_rel_for(&viewer, &owner).is_err());
    assert!(g.reopen_rel_for(&manager, &other).is_err());
    g.replace_instance_grants(&manager, BTreeMap::new())
        .unwrap();
    assert!(g.reopen_rel_for(&manager, &owner).is_err());
    assert_eq!(
        g.owner_admission(&owner).unwrap(),
        crate::OwnerAdmission::Closed
    );
    g.replace_instance_grants(&manager, grants(&owner, &[OwnerGrant::ManageRel]))
        .unwrap();
    g.reopen_rel_for(&manager, &owner).unwrap();
    g.set_rel_processing_enabled_for(&manager, &owner, false)
        .unwrap();
    g.detach_instance(&manager).unwrap();
    assert!(
        g.set_rel_processing_enabled_for(&manager, &owner, true)
            .is_err()
    );
    assert!(g.quiesce_rel_for(&manager, &owner).is_err());
    assert!(g.reopen_rel_for(&manager, &owner).is_err());
    assert!(g.reconcile_quiesced_rel_for(&manager, &owner).is_err());
    g.detach_instance(&viewer).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g.unmount_rel(&other).unwrap());
}
