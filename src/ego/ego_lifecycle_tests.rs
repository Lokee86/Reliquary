use super::test_path;
use crate::ego_codec::{EgoRecord, encode};
use crate::owner_lifecycle::{
    LifecycleError, LifecycleState, LifecycleTarget, TombstoneCoordinates,
};
use crate::{Cva, EgoAnchorPriority, EgoError, EgoIdentity, Phylactery, PhylacteryError};

#[test]
fn identity_deletion_is_terminal_clock_neutral_on_retry_and_keeps_history_after_reopen() {
    let path = test_path("identity-tombstone.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let identity = phy
        .create_ego_identity("Work".into(), "Use concise language.".into())
        .unwrap();
    let (revised, changed) = phy
        .update_ego_identity(
            identity.id,
            identity.revision,
            "Work".into(),
            "Use clear, concise language.".into(),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(revised.revision, 2);
    assert_eq!(
        phy.historical_ego_identity(identity.id, 1),
        Some(identity.clone())
    );

    assert!(
        phy.delete_ego_identity(revised.id, revised.revision)
            .unwrap()
    );
    let deleted_version = phy.ego_version();
    let deleted_state = LifecycleState::Tombstoned(TombstoneCoordinates {
        global_version: None,
        domain_version: Some(deleted_version),
    });
    assert_eq!(phy.ego_identity_lifecycle(identity.id), Some(deleted_state));
    assert_eq!(phy.ego_identity_by_id(identity.id), None);
    assert_eq!(phy.ego_identities(), Vec::<EgoIdentity>::new());
    assert_eq!(phy.active_ego_identity_id(), None);
    assert_eq!(
        phy.historical_ego_identity(identity.id, 2),
        Some(revised.clone())
    );
    assert_eq!(phy.historical_ego_identity(identity.id, 3), None);

    let terminal = LifecycleError::TerminalTombstone(LifecycleTarget::EgoIdentity(identity.id));
    assert!(matches!(
        phy.update_ego_identity(
            identity.id,
            revised.revision,
            "Work".into(),
            "Resurrected.".into()
        ),
        Err(PhylacteryError::Ego(EgoError::Lifecycle(error))) if error == terminal
    ));
    assert!(matches!(
        phy.activate_ego_identity(identity.id),
        Err(PhylacteryError::Ego(EgoError::Lifecycle(error))) if error == terminal
    ));

    let version_before_retry = phy.ego_version();
    assert!(
        !phy.delete_ego_identity(identity.id, revised.revision)
            .unwrap()
    );
    assert_eq!(phy.ego_version(), version_before_retry);
    assert!(matches!(
        phy.delete_ego_identity(identity.id, 99),
        Err(PhylacteryError::Ego(EgoError::RevisionConflict { .. }))
    ));
    assert_eq!(phy.ego_version(), version_before_retry);

    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(path).unwrap();
    assert_eq!(
        reopened.ego_identity_lifecycle(identity.id),
        Some(deleted_state)
    );
    assert_eq!(reopened.ego_identity_by_id(identity.id), None);
    assert_eq!(
        reopened.historical_ego_identity(identity.id, 1),
        Some(identity.clone())
    );
    assert_eq!(
        reopened.historical_ego_identity(identity.id, 2),
        Some(revised)
    );
}

#[test]
fn anchor_deletion_retains_historical_payload_and_retries_without_clock_changes_for_both_owners() {
    let phy_path = test_path("anchor-tombstone.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let anchor = phy
        .create_ego_anchor(EgoAnchorPriority::High, "PHY standing context.".into())
        .unwrap();
    let (revised, changed) = phy
        .update_ego_anchor(
            anchor.id,
            anchor.revision,
            EgoAnchorPriority::Normal,
            "Updated PHY standing context.".into(),
        )
        .unwrap();
    assert!(changed);
    assert!(phy.delete_ego_anchor(revised.id, revised.revision).unwrap());
    let deleted_version = phy.ego_version();
    let deleted_state = LifecycleState::Tombstoned(TombstoneCoordinates {
        global_version: None,
        domain_version: Some(deleted_version),
    });
    assert_eq!(phy.ego_anchor_lifecycle(anchor.id), Some(deleted_state));
    assert_eq!(phy.ego_anchor(anchor.id), None);
    assert_eq!(phy.ego_anchors(), Vec::new());
    assert_eq!(
        phy.historical_ego_anchor(anchor.id, 1),
        Some(anchor.clone())
    );
    assert_eq!(
        phy.historical_ego_anchor(anchor.id, 2),
        Some(revised.clone())
    );
    assert_eq!(phy.historical_ego_anchor(anchor.id, 3), None);
    let version_before_retry = phy.ego_version();
    assert!(!phy.delete_ego_anchor(anchor.id, revised.revision).unwrap());
    assert_eq!(phy.ego_version(), version_before_retry);
    assert!(matches!(
        phy.update_ego_anchor(
            anchor.id,
            revised.revision,
            EgoAnchorPriority::High,
            "Resurrected.".into()
        ),
        Err(PhylacteryError::Ego(EgoError::Lifecycle(
            LifecycleError::TerminalTombstone(LifecycleTarget::EgoAnchor(id))
        ))) if id == anchor.id
    ));
    phy.sync().unwrap();
    drop(phy);

    let reopened_phy = Phylactery::open(phy_path).unwrap();
    assert_eq!(
        reopened_phy.ego_anchor_lifecycle(anchor.id),
        Some(deleted_state)
    );
    assert_eq!(reopened_phy.ego_anchor(anchor.id), None);
    assert_eq!(
        reopened_phy.historical_ego_anchor(anchor.id, 1),
        Some(anchor)
    );
    assert_eq!(reopened_phy.historical_ego_anchor(revised.id, 3), None);

    let rel_path = test_path("anchor-tombstone.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    let rel_anchor = rel
        .create_ego_anchor(EgoAnchorPriority::Low, "REL standing context.".into())
        .unwrap();
    assert!(
        rel.delete_ego_anchor(rel_anchor.id, rel_anchor.revision)
            .unwrap()
    );
    let rel_deleted_version = rel.ego_version();
    let rel_deleted_state = LifecycleState::Tombstoned(TombstoneCoordinates {
        global_version: None,
        domain_version: Some(rel_deleted_version),
    });
    assert_eq!(
        rel.ego_anchor_lifecycle(rel_anchor.id),
        Some(rel_deleted_state)
    );
    assert_eq!(rel.ego_anchor(rel_anchor.id), None);
    assert_eq!(
        rel.historical_ego_anchor(rel_anchor.id, 1),
        Some(rel_anchor.clone())
    );
    assert_eq!(rel.historical_ego_anchor(rel_anchor.id, 2), None);
    let version_before_retry = rel.ego_version();
    assert!(
        !rel.delete_ego_anchor(rel_anchor.id, rel_anchor.revision)
            .unwrap()
    );
    assert_eq!(rel.ego_version(), version_before_retry);
    rel.sync().unwrap();
    drop(rel);

    let reopened_rel = Cva::open(rel_path).unwrap();
    assert_eq!(
        reopened_rel.ego_anchor_lifecycle(rel_anchor.id),
        Some(rel_deleted_state)
    );
    assert_eq!(reopened_rel.ego_anchor(rel_anchor.id), None);
    assert_eq!(
        reopened_rel.historical_ego_anchor(rel_anchor.id, 1),
        Some(rel_anchor)
    );
}

#[test]
fn replay_rejects_identity_and_anchor_revisions_that_follow_tombstones() {
    let identity_path = test_path("identity-invalid-replay.phy");
    let identity_id = crate::EgoIdentityId([7; 16]);
    let mut phy = Phylactery::create(&identity_path).unwrap();
    for record in [
        EgoRecord::Identity {
            ego_version: 1,
            id: identity_id,
            revision: 1,
            deleted: false,
            activate: true,
            name: "Default".into(),
            text: "Active identity.".into(),
        },
        EgoRecord::Identity {
            ego_version: 2,
            id: identity_id,
            revision: 2,
            deleted: true,
            activate: false,
            name: String::new(),
            text: String::new(),
        },
        EgoRecord::Identity {
            ego_version: 3,
            id: identity_id,
            revision: 3,
            deleted: false,
            activate: true,
            name: "Returned".into(),
            text: "This revision follows a terminal deletion.".into(),
        },
    ] {
        phy.container.append(&encode(&record).unwrap()).unwrap();
    }
    phy.sync().unwrap();
    drop(phy);
    assert!(matches!(
        Phylactery::open(identity_path),
        Err(PhylacteryError::Ego(EgoError::Lifecycle(
            LifecycleError::TerminalTombstone(LifecycleTarget::EgoIdentity(id))
        ))) if id == identity_id
    ));

    let anchor_path = test_path("anchor-invalid-replay.rel");
    let anchor_id = crate::EgoAnchorId([9; 16]);
    let mut rel = Cva::create(&anchor_path).unwrap();
    for record in [
        EgoRecord::Anchor {
            ego_version: 1,
            id: anchor_id,
            revision: 1,
            deleted: false,
            priority: EgoAnchorPriority::Normal,
            text: "Active anchor.".into(),
        },
        EgoRecord::Anchor {
            ego_version: 2,
            id: anchor_id,
            revision: 2,
            deleted: true,
            priority: EgoAnchorPriority::Normal,
            text: String::new(),
        },
        EgoRecord::Anchor {
            ego_version: 3,
            id: anchor_id,
            revision: 3,
            deleted: false,
            priority: EgoAnchorPriority::High,
            text: "This revision follows a terminal deletion.".into(),
        },
    ] {
        rel.container.append(&encode(&record).unwrap()).unwrap();
    }
    rel.sync().unwrap();
    drop(rel);
    assert!(matches!(
        Cva::open(anchor_path),
        Err(crate::CvaError::Ego(EgoError::Lifecycle(
            LifecycleError::TerminalTombstone(LifecycleTarget::EgoAnchor(id))
        ))) if id == anchor_id
    ));
}
