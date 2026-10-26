use crate::ego_codec::EgoRecord;
use crate::ego_compact_contract::{
    EgoChanges, EgoCreation, EgoDelta, EgoTerminal, is_terminal, revision,
};
use crate::ego_store::EgoStore;
use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};
use reliquary_container::codec::FieldChange;

fn identity(
    version: u64,
    id: EgoIdentityId,
    revision: u64,
    activate: bool,
    text: &str,
) -> EgoRecord {
    EgoRecord::Identity {
        ego_version: version,
        id,
        revision,
        deleted: false,
        activate,
        name: "Default".into(),
        text: text.into(),
    }
}

fn anchor(
    version: u64,
    id: EgoAnchorId,
    revision: u64,
    deleted: bool,
    priority: EgoAnchorPriority,
    text: &str,
) -> EgoRecord {
    EgoRecord::Anchor {
        ego_version: version,
        id,
        revision,
        deleted,
        priority,
        text: text.into(),
    }
}

fn invalid<T>(result: Result<T, EgoError>) {
    assert!(matches!(result, Err(EgoError::InvalidRecord(_))));
}

#[test]
fn raw_identical_postimage_is_a_zero_field_delta_but_keeps_publication() {
    let id = EgoIdentityId([1; 16]);
    let prior = identity(4, id, 7, false, "same");
    let next = identity(5, id, 8, false, "same");
    let delta = EgoDelta::between(&prior, &next).unwrap();
    assert_eq!(delta.ego_version, 5);
    assert_eq!((delta.previous_revision, delta.revision), (7, 8));
    assert_eq!(
        delta.changes,
        EgoChanges::Identity {
            id,
            activate: false,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        }
    );
}

#[test]
fn identity_update_delta_preserves_activation_metadata() {
    let id = EgoIdentityId([2; 16]);
    let prior = identity(1, id, 1, true, "before");
    let next = identity(2, id, 2, true, "after");
    assert_eq!(
        EgoDelta::between(&prior, &next).unwrap().changes,
        EgoChanges::Identity {
            id,
            activate: true,
            name: FieldChange::Unchanged,
            text: FieldChange::Set("after".into()),
        }
    );
}

#[test]
fn delta_rejects_wrong_id_kind_revision_clock_and_terminal_base() {
    let id = EgoIdentityId([3; 16]);
    let prior = identity(1, id, 1, false, "before");
    invalid(EgoDelta::between(
        &prior,
        &identity(2, EgoIdentityId([4; 16]), 2, false, "after"),
    ));
    invalid(EgoDelta::between(
        &prior,
        &EgoRecord::Synthesis {
            ego_version: 2,
            revision: 2,
            source_memory_version: 1,
            text: "after".into(),
        },
    ));
    invalid(EgoDelta::between(
        &prior,
        &identity(2, id, 3, false, "after"),
    ));
    invalid(EgoDelta::between(
        &prior,
        &identity(1, id, 2, false, "after"),
    ));
    invalid(EgoDelta::between(
        &prior,
        &identity(0, id, 2, false, "after"),
    ));
    let terminal = EgoRecord::Identity {
        ego_version: 2,
        id,
        revision: 2,
        deleted: true,
        activate: false,
        name: String::new(),
        text: String::new(),
    };
    invalid(EgoDelta::between(
        &terminal,
        &identity(3, id, 3, false, "resurrected"),
    ));
    invalid(EgoDelta::between(&prior, &terminal));
}

#[test]
fn terminal_creation_accepts_first_revision_and_anchor_priority_is_retained() {
    let id = EgoAnchorId([5; 16]);
    let terminal = anchor(1, id, 1, true, EgoAnchorPriority::High, "");
    assert!(is_terminal(&terminal));
    assert_eq!(EgoTerminal::new(terminal.clone()).unwrap().record, terminal);
    let prior = anchor(2, id, 1, false, EgoAnchorPriority::Low, "standing context");
    let next = anchor(3, id, 2, true, EgoAnchorPriority::High, "");
    assert!(matches!(
        EgoDelta::between(&prior, &next),
        Err(EgoError::InvalidRecord(_))
    ));
    assert_eq!(EgoTerminal::new(next.clone()).unwrap().record, next);
}

#[test]
fn document_deltas_allow_decreasing_source_watermark_and_capture_metadata_only_change() {
    let prior = EgoRecord::Synthesis {
        ego_version: 7,
        revision: 4,
        source_memory_version: 12,
        text: "same synthesis".into(),
    };
    let next = EgoRecord::Synthesis {
        ego_version: 8,
        revision: 5,
        source_memory_version: 9,
        text: "same synthesis".into(),
    };
    assert_eq!(
        EgoDelta::between(&prior, &next).unwrap().changes,
        EgoChanges::Synthesis {
            source_memory_version: FieldChange::Set(9),
            text: FieldChange::Unchanged,
        }
    );
}

#[test]
fn raw_ingest_preserves_repeated_publications_and_owner_restrictions() {
    let id = EgoIdentityId([6; 16]);
    let other_id = EgoIdentityId([9; 16]);
    let first = identity(1, id, 1, true, "same");
    let second = identity(2, id, 2, true, "same");
    let mut phy = EgoStore::phylactery();
    phy.ingest(&crate::ego_current_fixture::encode_postimage(&first).unwrap())
        .unwrap();
    phy.ingest(&phy.encode_compact_publication(&second).unwrap())
        .unwrap();
    phy.ingest(
        &crate::ego_current_fixture::encode_postimage(&identity(3, other_id, 1, false, "other"))
            .unwrap(),
    )
    .unwrap();
    phy.ingest(
        &phy.encode_compact_publication(&identity(4, other_id, 2, true, "other revised"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(phy.version(), 4);
    assert_eq!(phy.active_identity_id(), Some(other_id));
    assert_eq!(phy.identity(id).unwrap().revision, 2);

    let mut rel = EgoStore::reliquary();
    assert!(matches!(
        rel.ingest(&crate::ego_current_fixture::encode_postimage(&first).unwrap()),
        Err(EgoError::InvalidOwnerRecord(_))
    ));
}

#[test]
fn raw_anchor_terminal_preserves_changed_priority_and_can_be_first_revision() {
    let id = EgoAnchorId([10; 16]);
    let mut store = EgoStore::reliquary();
    let live = anchor(1, id, 1, false, EgoAnchorPriority::Low, "standing context");
    let deleted = anchor(2, id, 2, true, EgoAnchorPriority::High, "");
    store
        .ingest(&crate::ego_current_fixture::encode_postimage(&live).unwrap())
        .unwrap();
    store
        .ingest(&crate::ego_current_fixture::encode_postimage(&deleted).unwrap())
        .unwrap();
    assert_eq!(store.version(), 2);
    assert!(store.anchor(id).is_none());
    assert!(matches!(
        store.anchor_lifecycle(id),
        Some(crate::owner_lifecycle::LifecycleState::Tombstoned(_))
    ));

    let initial_terminal = anchor(
        1,
        EgoAnchorId([11; 16]),
        1,
        true,
        EgoAnchorPriority::Normal,
        "",
    );
    let mut another = EgoStore::reliquary();
    another
        .ingest(&crate::ego_current_fixture::encode_postimage(&initial_terminal).unwrap())
        .unwrap();
    assert_eq!(another.version(), 1);
}

#[test]
fn active_identity_deletion_requires_swapping_when_another_identity_is_live() {
    let first_id = EgoIdentityId([7; 16]);
    let second_id = EgoIdentityId([8; 16]);
    let mut store = EgoStore::phylactery();
    store
        .ingest(
            &crate::ego_current_fixture::encode_postimage(&identity(1, first_id, 1, true, "one"))
                .unwrap(),
        )
        .unwrap();
    store
        .ingest(
            &crate::ego_current_fixture::encode_postimage(&identity(2, second_id, 1, false, "two"))
                .unwrap(),
        )
        .unwrap();
    let deleted = EgoRecord::Identity {
        ego_version: 3,
        id: first_id,
        revision: 2,
        deleted: true,
        activate: false,
        name: String::new(),
        text: String::new(),
    };
    assert!(matches!(
        store.ingest(&crate::ego_current_fixture::encode_postimage(&deleted).unwrap()),
        Err(EgoError::ActiveIdentityDeletion)
    ));
    assert_eq!(store.version(), 2);
}
