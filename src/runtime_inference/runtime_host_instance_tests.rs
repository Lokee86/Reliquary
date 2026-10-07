use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn grants(rel_ids: &[&str]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    rel_ids
        .iter()
        .map(|rel_id| {
            (
                (*rel_id).to_owned(),
                BTreeSet::from([OwnerGrant::ViewRel, OwnerGrant::SubmitConversation]),
            )
        })
        .collect()
}

#[test]
fn active_rel_and_managed_conversation_are_instance_local() {
    let first = InstanceRuntime::new("principal-a".into(), None, grants(&["rel-a", "rel-b"]));
    let second = InstanceRuntime::new("principal-b".into(), None, grants(&["rel-a", "rel-b"]));

    assert!(first.select_rel("rel-a").unwrap());
    assert!(second.select_rel("rel-b").unwrap());
    first
        .set_managed_conversation_state(
            "rel-a",
            ManagedConversationState {
                conversation_id: Some("conversation-a".into()),
                branch_id: Some("branch-a".into()),
                selected_leaf_node_id: None,
                reopened_session_pending: true,
            },
        )
        .unwrap();

    assert_eq!(first.selected_rel().unwrap().as_deref(), Some("rel-a"));
    assert_eq!(second.selected_rel().unwrap().as_deref(), Some("rel-b"));
    assert_eq!(
        first.managed_conversation_state("rel-a").unwrap(),
        ManagedConversationState {
            conversation_id: Some("conversation-a".into()),
            branch_id: Some("branch-a".into()),
            selected_leaf_node_id: None,
            reopened_session_pending: true,
        }
    );
    assert_eq!(
        second.managed_conversation_state("rel-a").unwrap(),
        ManagedConversationState::default()
    );
}

#[test]
fn per_rel_managed_selection_survives_switching_rel() {
    let instance = InstanceRuntime::new("principal".into(), None, grants(&["rel-a", "rel-b"]));
    instance.select_rel("rel-a").unwrap();
    instance
        .set_managed_conversation_state(
            "rel-a",
            ManagedConversationState {
                conversation_id: Some("session-a".into()),
                branch_id: Some("branch-a".into()),
                selected_leaf_node_id: None,
                reopened_session_pending: false,
            },
        )
        .unwrap();

    instance.select_rel("rel-b").unwrap();
    instance
        .set_managed_conversation_state(
            "rel-b",
            ManagedConversationState {
                conversation_id: Some("session-b".into()),
                branch_id: Some("branch-b".into()),
                selected_leaf_node_id: None,
                reopened_session_pending: true,
            },
        )
        .unwrap();
    instance.select_rel("rel-a").unwrap();

    assert_eq!(
        instance
            .managed_conversation_state("rel-a")
            .unwrap()
            .conversation_id
            .as_deref(),
        Some("session-a")
    );
    assert!(
        !instance
            .managed_conversation_state("rel-a")
            .unwrap()
            .reopened_session_pending
    );
    assert_eq!(
        instance
            .managed_conversation_state("rel-b")
            .unwrap()
            .conversation_id
            .as_deref(),
        Some("session-b")
    );
    assert!(
        instance
            .managed_conversation_state("rel-b")
            .unwrap()
            .reopened_session_pending
    );
}

#[test]
fn ambient_selection_requires_rel_view_grant() {
    let instance = InstanceRuntime::new("principal".into(), None, grants(&["rel-a"]));

    assert!(matches!(
        instance.select_rel("rel-ungranted"),
        Err(InstanceRuntimeError::GrantDenied {
            grant: OwnerGrant::ViewRel,
            ..
        })
    ));
    assert_eq!(instance.selected_rel().unwrap(), None);
}

#[test]
fn stale_authorization_is_rejected_after_detach_and_detach_is_repeatable() {
    let instance = InstanceRuntime::new("principal".into(), None, grants(&["rel-a"]));
    let captured = instance.authorization().unwrap();
    instance.select_rel("rel-a").unwrap();

    assert!(instance.detach().unwrap());
    assert!(!instance.detach().unwrap());
    assert!(matches!(
        instance.authorize("rel-a", OwnerGrant::ViewRel, captured),
        Err(InstanceRuntimeError::Detached)
    ));
    assert!(matches!(
        instance.selected_rel(),
        Err(InstanceRuntimeError::Detached)
    ));
    assert!(!instance.is_attached().unwrap());
}

#[test]
fn phy_grant_is_bound_to_the_attached_owner() {
    let mut owner_grants = grants(&["rel-a"]);
    owner_grants.insert("phy-a".into(), BTreeSet::from([OwnerGrant::UseOwnPhy]));
    owner_grants.insert("phy-b".into(), BTreeSet::from([OwnerGrant::UseOwnPhy]));
    let instance = InstanceRuntime::new("principal".into(), Some("phy-a".into()), owner_grants);
    let authorization = instance.authorization().unwrap();

    assert_eq!(instance.attached_phy_owner_id(), Some("phy-a"));
    instance
        .authorize("phy-a", OwnerGrant::UseOwnPhy, authorization)
        .unwrap();
    assert!(matches!(
        instance.authorize("phy-b", OwnerGrant::UseOwnPhy, authorization),
        Err(InstanceRuntimeError::GrantDenied {
            grant: OwnerGrant::UseOwnPhy,
            ..
        })
    ));
}

#[test]
fn authorization_cannot_be_replayed_across_instances() {
    let one = InstanceRuntime::new("principal".into(), None, grants(&["rel-a"]));
    let two = InstanceRuntime::new("principal".into(), None, grants(&["rel-a"]));
    let token = one.authorization().unwrap();

    assert!(matches!(
        two.authorize("rel-a", OwnerGrant::ViewRel, token),
        Err(InstanceRuntimeError::StaleAuthorization)
    ));
}

#[test]
fn grant_replacement_invalidates_token_and_clears_unauthorized_selection() {
    let instance = InstanceRuntime::new("principal".into(), None, grants(&["rel-a"]));
    let captured = instance.authorization().unwrap();
    instance.select_rel("rel-a").unwrap();

    instance.replace_grants(BTreeMap::new()).unwrap();

    assert!(matches!(
        instance.authorize("rel-a", OwnerGrant::ViewRel, captured),
        Err(InstanceRuntimeError::StaleAuthorization)
    ));
    assert_eq!(instance.selected_rel().unwrap(), None);
    assert!(matches!(
        instance.select_rel("rel-a"),
        Err(InstanceRuntimeError::GrantDenied { .. })
    ));
}
