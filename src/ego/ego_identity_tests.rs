use super::test_path;
use crate::ego_codec::EgoRecord;
use crate::ego_current_fixture::encode_postimage;
use crate::{EgoError, Phylactery, PhylacteryError};

#[test]
fn multiple_identities_have_one_swappable_active_identity() {
    let path = test_path("multiple-identities.phy");
    let mut phy = Phylactery::create(&path).unwrap();

    let jarvis = phy
        .create_ego_identity("Jarvis".into(), "You are Jarvis.".into())
        .unwrap();
    assert_eq!(phy.active_ego_identity_id(), Some(jarvis.id));
    assert_eq!(phy.ego_identity().unwrap(), jarvis);

    let optimus = phy
        .create_ego_identity("Optimus".into(), "You are Optimus Prime.".into())
        .unwrap();
    assert_eq!(phy.ego_identities().len(), 2);
    assert_eq!(phy.active_ego_identity_id(), Some(jarvis.id));

    let version = phy.ego_version();
    assert!(phy.activate_ego_identity(optimus.id).unwrap());
    assert_eq!(phy.active_ego_identity_id(), Some(optimus.id));
    assert_eq!(phy.ego_identity().unwrap(), optimus);
    assert_eq!(phy.ego_version(), version + 1);

    let version = phy.ego_version();
    assert!(!phy.activate_ego_identity(optimus.id).unwrap());
    assert_eq!(phy.ego_version(), version);

    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(path).unwrap();
    assert_eq!(reopened.ego_identities().len(), 2);
    assert_eq!(reopened.active_ego_identity_id(), Some(optimus.id));
    assert_eq!(reopened.ego_identity().unwrap().name, "Optimus");
}

#[test]
fn identities_update_independently_and_active_deletion_requires_swap() {
    let path = test_path("identity-lifecycle.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let first = phy
        .create_ego_identity("Default".into(), "You are Aster.".into())
        .unwrap();
    let second = phy
        .create_ego_identity("Pirate".into(), "You are a pirate.".into())
        .unwrap();

    let (updated, changed) = phy
        .update_ego_identity(
            second.id,
            second.revision,
            "Pirate".into(),
            "You are a concise pirate.".into(),
        )
        .unwrap();
    assert!(changed);
    assert_eq!(updated.revision, 2);
    assert_eq!(phy.ego_identity().unwrap().id, first.id);

    assert!(matches!(
        phy.delete_ego_identity(first.id, first.revision),
        Err(PhylacteryError::Ego(EgoError::ActiveIdentityDeletion))
    ));
    phy.activate_ego_identity(updated.id).unwrap();
    assert!(phy.delete_ego_identity(first.id, first.revision).unwrap());
    assert_eq!(phy.ego_identities(), vec![updated.clone()]);
    assert_eq!(phy.ego_identity().unwrap(), updated);
}

#[test]
fn deleting_only_identity_leaves_empty_identity_set() {
    let path = test_path("delete-only-identity.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let identity = phy
        .create_ego_identity("Temporary".into(), "Temporary identity.".into())
        .unwrap();
    assert!(
        phy.delete_ego_identity(identity.id, identity.revision)
            .unwrap()
    );
    assert!(phy.ego_identities().is_empty());
    assert!(phy.ego_identity().is_none());
    assert_eq!(phy.active_ego_identity_id(), None);
}
