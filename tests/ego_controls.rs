use reliquary_memory::{
    EgoIdentityDraft, EgoPinPatch, EgoPinPriority, EgoScope, Gateway, HostCredentialBinding,
    OwnerGrant, OwnerKind, Phylactery, Rel, TrustedHostAdmission,
};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("ego-controls-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn mounted_controls_keep_pins_addressable_and_bind_approval_to_exact_identity() {
    let fixture = Fixture::new();
    let gateway = Gateway::new();
    let rel = gateway
        .mount_rel(&Rel::create(fixture.0.join("work.rel")).unwrap(), true)
        .unwrap();
    let phy_path = fixture.0.join("person.phy");
    let phy = gateway
        .mount_phylactery(&Phylactery::create(&phy_path).unwrap(), true)
        .unwrap();
    const EDIT: &str = "ego-edit-fixture-local-authority-0001";
    const VIEW: &str = "ego-view-fixture-local-authority-0001";
    let (auth, control) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "editor",
            EDIT,
            [
                OwnerGrant::OperateRel(rel),
                OwnerGrant::OperatePhylactery(phy),
            ],
        ),
        HostCredentialBinding::new(
            "viewer",
            VIEW,
            [OwnerGrant::ViewRel(rel), OwnerGrant::ViewPhylactery(phy)],
        ),
    ])
    .install(&gateway)
    .unwrap();
    let editor = auth.authenticate(EDIT).unwrap();
    let viewer = auth.authenticate(VIEW).unwrap();
    let rel_scope = EgoScope::Rel(rel);
    let phy_scope = EgoScope::Phy(phy);
    let lower = editor
        .pin_anchor(
            rel_scope,
            "Keep a local changelog.".into(),
            Some(EgoPinPriority::Low),
        )
        .unwrap();
    let higher = editor
        .pin_anchor(
            phy_scope,
            "Use metric units.".into(),
            Some(EgoPinPriority::High),
        )
        .unwrap();
    let input = viewer
        .ego_context(Some(rel), phy, "", None, 40, str::len)
        .unwrap();
    assert!(input.standing_context.contains(&higher.text));
    assert!(!input.standing_context.contains(&lower.text));
    assert_eq!(viewer.ego_pins(rel_scope).unwrap(), [lower.clone()]);
    let patch = EgoPinPatch {
        text: None,
        priority: Some(EgoPinPriority::Normal),
    };
    assert!(
        viewer
            .modify_anchor(rel_scope, lower.id, lower.revision, patch.clone())
            .is_err()
    );
    let edited = editor
        .modify_anchor(rel_scope, lower.id, lower.revision, patch.clone())
        .unwrap();
    assert_eq!(edited.text, lower.text);
    assert_eq!(edited.priority, EgoPinPriority::Normal);
    assert!(
        editor
            .modify_anchor(rel_scope, lower.id, lower.revision, patch)
            .is_err()
    );

    let old = editor
        .create_ego_identity(phy, "Editorial".into(), "Use plain language.".into())
        .unwrap();
    editor
        .select_ego_identity(phy, 0, Some((old.id, old.revision)))
        .unwrap();
    let draft = EgoIdentityDraft {
        id: old.id,
        revision: old.revision,
        name: old.name.clone(),
        text: "Explain tradeoffs plainly.".into(),
    };
    let prepared = editor.prepare_ego_identity(phy, draft.clone()).unwrap();
    let for_other_credential = editor.prepare_ego_identity(phy, draft).unwrap();
    assert!(
        viewer
            .approve_ego_identity(phy, for_other_credential)
            .is_err()
    );
    assert_eq!(
        viewer
            .ego_context(Some(rel), phy, "", None, 200, str::len)
            .unwrap()
            .identity,
        Some(old.text)
    );
    let changed = editor.approve_ego_identity(phy, prepared).unwrap();
    assert_eq!(
        viewer
            .ego_context(Some(rel), phy, "", None, 200, str::len)
            .unwrap()
            .identity,
        Some(changed.text)
    );
    assert!(control.quiesce_writable_phylactery(&viewer, phy).is_err());
    let (other_admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "alternate editor",
        "ego-edit-fixture-separate-authorization-0002",
        [OwnerGrant::OperatePhylactery(phy)],
    )])
    .install(&gateway)
    .unwrap();
    let outsider = other_admission
        .authenticate("ego-edit-fixture-separate-authorization-0002")
        .unwrap();
    assert!(control.quiesce_writable_phylactery(&outsider, phy).is_err());
    control.quiesce_writable_phylactery(&editor, phy).unwrap();
    assert_eq!(viewer.ego_pins(rel_scope).unwrap(), [edited]);
    assert!(viewer.ego_pins(phy_scope).is_err());
    assert_eq!(
        gateway
            .mount_owner_path(&phy_path, OwnerKind::Phylactery, true, Some(phy))
            .unwrap(),
        phy
    );
    assert_eq!(viewer.ego_pins(phy_scope).unwrap(), [higher]);
    control.revoke(VIEW).unwrap();
    assert!(
        viewer
            .ego_context(Some(rel), phy, "", None, 200, str::len)
            .is_err()
    );
}
