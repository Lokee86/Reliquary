use crate::compact_delta::FieldChange;
use crate::ego_codec::{self, EgoRecord, compact};
use crate::ego_compact_contract::{EgoChanges, EgoCreation, EgoDelta, EgoTerminal};
use crate::ego_store::EgoStore;
use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId, FormatVersion};

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

fn creation(record: EgoRecord) -> Vec<u8> {
    compact::encode_creation(&EgoCreation::new(record).unwrap()).unwrap()
}

fn delta(previous: &EgoRecord, next: &EgoRecord) -> Vec<u8> {
    compact::encode_delta(&EgoDelta::between(previous, next).unwrap()).unwrap()
}

fn terminal(record: EgoRecord) -> Vec<u8> {
    compact::encode_terminal(&EgoTerminal::new(record).unwrap()).unwrap()
}

fn invalid<T>(result: Result<T, EgoError>) {
    assert!(matches!(result, Err(EgoError::InvalidRecord(_))));
}

#[test]
fn interleaved_rel_and_phy_compact_histories_match_legacy_replay_publications() {
    let anchor_id = EgoAnchorId([0x11; 16]);
    let identity_id = EgoIdentityId([0x22; 16]);

    // REL interleaves Anchor and Synthesis publications. Each compact result
    // must equal the original admitted legacy postimage, including its clock.
    let mut rel_compact = EgoStore::reliquary();
    let mut rel_legacy = EgoStore::reliquary();
    let rel_a1 = anchor(
        1,
        anchor_id,
        1,
        false,
        EgoAnchorPriority::Low,
        "first anchor",
    );
    let rel_s1 = EgoRecord::Synthesis {
        ego_version: 2,
        revision: 1,
        source_memory_version: 8,
        text: "first synthesis".into(),
    };
    let rel_a2 = anchor(
        3,
        anchor_id,
        2,
        false,
        EgoAnchorPriority::High,
        "revised anchor",
    );
    let rel_s2 = EgoRecord::Synthesis {
        ego_version: 4,
        revision: 2,
        source_memory_version: 5,
        text: "revised synthesis".into(),
    };
    for (record, payload) in [
        (rel_a1.clone(), creation(rel_a1.clone())),
        (rel_s1.clone(), creation(rel_s1.clone())),
        (rel_a2.clone(), delta(&rel_a1, &rel_a2)),
        (rel_s2.clone(), delta(&rel_s1, &rel_s2)),
    ] {
        assert_eq!(
            rel_compact.ingest_publication(&payload).unwrap(),
            Some(record.clone())
        );
        rel_legacy
            .ingest(&ego_codec::encode(&record).unwrap())
            .unwrap();
    }
    assert_eq!(rel_compact.version(), rel_legacy.version());
    assert_eq!(rel_compact.anchor(anchor_id), rel_legacy.anchor(anchor_id));
    assert_eq!(rel_compact.synthesis(), rel_legacy.synthesis());

    // PHY interleaves Identity and Personality. The raw identity update both
    // changes its text and activates it; Personality's source cut decreases.
    let mut phy_compact = EgoStore::phylactery();
    let mut phy_legacy = EgoStore::phylactery();
    let phy_i1 = identity(1, identity_id, 1, true, "first identity");
    let phy_p1 = EgoRecord::Personality {
        ego_version: 2,
        revision: 1,
        source_memory_version: 9,
        text: "first personality".into(),
    };
    let phy_i2 = identity(3, identity_id, 2, true, "revised identity");
    let phy_p2 = EgoRecord::Personality {
        ego_version: 4,
        revision: 2,
        source_memory_version: 4,
        text: "revised personality".into(),
    };
    for (record, payload) in [
        (phy_i1.clone(), creation(phy_i1.clone())),
        (phy_p1.clone(), creation(phy_p1.clone())),
        (phy_i2.clone(), delta(&phy_i1, &phy_i2)),
        (phy_p2.clone(), delta(&phy_p1, &phy_p2)),
    ] {
        assert_eq!(
            phy_compact.ingest_publication(&payload).unwrap(),
            Some(record.clone())
        );
        phy_legacy
            .ingest(&ego_codec::encode(&record).unwrap())
            .unwrap();
    }
    assert_eq!(phy_compact.version(), phy_legacy.version());
    assert_eq!(
        phy_compact.active_identity_id(),
        phy_legacy.active_identity_id()
    );
    assert_eq!(
        phy_compact.identity(identity_id),
        phy_legacy.identity(identity_id)
    );
    assert_eq!(phy_compact.personality(), phy_legacy.personality());
}

#[test]
fn compact_history_preserves_activation_only_and_zero_change_publications() {
    let first_id = EgoIdentityId([0x31; 16]);
    let second_id = EgoIdentityId([0x32; 16]);
    let mut store = EgoStore::phylactery();
    let first = identity(1, first_id, 1, true, "first");
    let second = identity(2, second_id, 1, false, "second");
    store.ingest_publication(&creation(first.clone())).unwrap();
    store.ingest_publication(&creation(second.clone())).unwrap();

    let activation = identity(3, second_id, 2, true, "second");
    let activation_bytes = delta(&second, &activation);
    assert_eq!(
        store.ingest_publication(&activation_bytes).unwrap(),
        Some(activation)
    );
    assert_eq!(store.active_identity_id(), Some(second_id));

    let repeated = identity(4, second_id, 3, true, "second");
    assert_eq!(
        store
            .ingest_publication(&delta(
                &identity(3, second_id, 2, true, "second"),
                &repeated
            ))
            .unwrap(),
        Some(repeated)
    );
    assert_eq!(store.version(), 4);

    // A zero-field Delta's activation bit is still explicit publication data.
    let no_activation = EgoDelta {
        ego_version: 5,
        previous_revision: 3,
        revision: 4,
        changes: EgoChanges::Identity {
            id: second_id,
            activate: false,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        },
    };
    let bytes = compact::encode_delta(&no_activation).unwrap();
    let materialized = store.ingest_publication(&bytes).unwrap().unwrap();
    assert_eq!(materialized, identity(5, second_id, 4, false, "second"));
    assert_eq!(store.version(), 5);

    // The admitted legacy minimal carrier normalizes to the stable Default
    // Identity and can immediately serve as a compact delta base.
    let legacy_id = crate::ego_store::LEGACY_IDENTITY_ID;
    let mut legacy_store = EgoStore::phylactery();
    legacy_store
        .ingest(
            &ego_codec::encode(&EgoRecord::LegacyIdentity {
                ego_version: 1,
                revision: 1,
                text: "legacy default".into(),
            })
            .unwrap(),
        )
        .unwrap();
    let activation_only = EgoDelta {
        ego_version: 2,
        previous_revision: 1,
        revision: 2,
        changes: EgoChanges::Identity {
            id: legacy_id,
            activate: true,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        },
    };
    assert_eq!(
        legacy_store
            .ingest_publication(&compact::encode_delta(&activation_only).unwrap())
            .unwrap(),
        Some(identity(2, legacy_id, 2, true, "legacy default"))
    );
}

#[test]
fn deletion_frames_retain_changed_anchor_priority_and_allow_initial_terminal() {
    let id = EgoAnchorId([0x41; 16]);
    let live = anchor(1, id, 1, false, EgoAnchorPriority::Low, "anchor");
    let deleted = anchor(2, id, 2, true, EgoAnchorPriority::High, "");
    let mut store = EgoStore::reliquary();
    store.ingest_publication(&creation(live)).unwrap();
    assert_eq!(
        store
            .ingest_publication(&terminal(deleted.clone()))
            .unwrap(),
        Some(deleted)
    );
    assert_eq!(store.version(), 2);
    assert!(store.anchor(id).is_none());

    let initial = anchor(
        1,
        EgoAnchorId([0x42; 16]),
        1,
        true,
        EgoAnchorPriority::Normal,
        "",
    );
    let mut empty = EgoStore::reliquary();
    assert_eq!(
        empty
            .ingest_publication(&terminal(initial.clone()))
            .unwrap(),
        Some(initial)
    );
    assert_eq!(empty.version(), 1);
}

#[test]
fn compact_replay_rejects_missing_wrong_terminal_owner_and_sequence_bases() {
    let id = EgoAnchorId([0x51; 16]);
    let absent_base = EgoDelta {
        ego_version: 1,
        previous_revision: 1,
        revision: 2,
        changes: EgoChanges::Anchor {
            id,
            priority: FieldChange::Unchanged,
            text: FieldChange::Set("new".into()),
        },
    };
    let mut empty = EgoStore::reliquary();
    invalid(empty.ingest_publication(&compact::encode_delta(&absent_base).unwrap()));

    let live = anchor(1, id, 1, false, EgoAnchorPriority::Normal, "before");
    let mut store = EgoStore::reliquary();
    store.ingest_publication(&creation(live.clone())).unwrap();
    let wrong_revision = EgoDelta {
        ego_version: 2,
        previous_revision: 2,
        revision: 3,
        changes: EgoChanges::Anchor {
            id,
            priority: FieldChange::Unchanged,
            text: FieldChange::Set("after".into()),
        },
    };
    invalid(store.ingest_publication(&compact::encode_delta(&wrong_revision).unwrap()));
    let gap = EgoDelta {
        ego_version: 3,
        previous_revision: 1,
        revision: 2,
        changes: EgoChanges::Anchor {
            id,
            priority: FieldChange::Unchanged,
            text: FieldChange::Set("after".into()),
        },
    };
    invalid(store.ingest_publication(&compact::encode_delta(&gap).unwrap()));
    assert_eq!(store.version(), 1);

    let deleted = anchor(2, id, 2, true, EgoAnchorPriority::High, "");
    store.ingest_publication(&terminal(deleted)).unwrap();
    let resurrect = EgoDelta {
        ego_version: 3,
        previous_revision: 2,
        revision: 3,
        changes: EgoChanges::Anchor {
            id,
            priority: FieldChange::Set(EgoAnchorPriority::Low),
            text: FieldChange::Unchanged,
        },
    };
    invalid(store.ingest_publication(&compact::encode_delta(&resurrect).unwrap()));

    let identity_id = EgoIdentityId([0x52; 16]);
    let mut rel = EgoStore::reliquary();
    rel.ingest_publication(&creation(EgoRecord::Synthesis {
        ego_version: 1,
        revision: 1,
        source_memory_version: 0,
        text: "synthesis".into(),
    }))
    .unwrap();
    let phy_only = identity(2, identity_id, 1, true, "identity");
    assert!(matches!(
        rel.ingest_publication(&creation(phy_only)),
        Err(EgoError::InvalidOwnerRecord(_))
    ));
    assert_eq!(rel.version(), 1);
}

#[test]
fn compact_publications_require_storage_v2_after_first_compact_frame() {
    let record = anchor(
        1,
        EgoAnchorId([0x61; 16]),
        1,
        false,
        EgoAnchorPriority::Normal,
        "context",
    );
    let mut store = EgoStore::reliquary();
    store.ingest_publication(&creation(record)).unwrap();
    assert!(matches!(
        store.validate_storage_version(FormatVersion { major: 1, minor: 0 }),
        Err(EgoError::InvalidRecord(_))
    ));
    assert!(
        store
            .validate_storage_version(FormatVersion { major: 2, minor: 0 })
            .is_ok()
    );

    let mut legacy_only = EgoStore::reliquary();
    legacy_only
        .ingest(
            &ego_codec::encode(&EgoRecord::Synthesis {
                ego_version: 1,
                revision: 1,
                source_memory_version: 0,
                text: "legacy".into(),
            })
            .unwrap(),
        )
        .unwrap();
    assert!(
        legacy_only
            .validate_storage_version(FormatVersion { major: 1, minor: 0 })
            .is_ok()
    );
}
