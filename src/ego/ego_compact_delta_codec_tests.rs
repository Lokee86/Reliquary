use crate::compact_delta::FieldChange;
use crate::ego_codec::{EgoRecord, compact};
use crate::ego_compact_contract::{EgoChanges, EgoDelta};
use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};

fn identity(
    version: u64,
    revision: u64,
    id: EgoIdentityId,
    activate: bool,
    name: &str,
    text: &str,
) -> EgoRecord {
    EgoRecord::Identity {
        ego_version: version,
        id,
        revision,
        deleted: false,
        activate,
        name: name.into(),
        text: text.into(),
    }
}

fn anchor(
    version: u64,
    revision: u64,
    id: EgoAnchorId,
    priority: EgoAnchorPriority,
    text: &str,
) -> EgoRecord {
    EgoRecord::Anchor {
        ego_version: version,
        id,
        revision,
        deleted: false,
        priority,
        text: text.into(),
    }
}

fn invalid<T>(result: Result<T, EgoError>) {
    assert!(matches!(result, Err(EgoError::InvalidRecord(_))));
}

#[test]
fn delta_roundtrips_all_four_kinds_and_masks() {
    let id = EgoIdentityId([0x11; 16]);
    let aid = EgoAnchorId([0x22; 16]);
    let pairs = [
        (
            identity(4, 7, id, false, "Before", "Old"),
            identity(5, 8, id, true, "After", "New"),
        ),
        (
            anchor(6, 3, aid, EgoAnchorPriority::Low, "Before"),
            anchor(7, 4, aid, EgoAnchorPriority::High, "After"),
        ),
        (
            EgoRecord::Personality {
                ego_version: 8,
                revision: 2,
                source_memory_version: 12,
                text: "Before".into(),
            },
            EgoRecord::Personality {
                ego_version: 9,
                revision: 3,
                source_memory_version: 15,
                text: "After".into(),
            },
        ),
        (
            EgoRecord::Synthesis {
                ego_version: 10,
                revision: 5,
                source_memory_version: 17,
                text: "Before".into(),
            },
            EgoRecord::Synthesis {
                ego_version: 11,
                revision: 6,
                source_memory_version: 18,
                text: "After".into(),
            },
        ),
    ];

    for (previous, next) in pairs {
        let expected = EgoDelta::between(&previous, &next).unwrap();
        let bytes = compact::encode_delta(&expected).unwrap();
        assert_eq!(compact::decode_delta(&bytes).unwrap(), expected);
    }

    let all_unchanged = EgoDelta {
        ego_version: 2,
        previous_revision: 1,
        revision: 2,
        changes: EgoChanges::Identity {
            id,
            activate: false,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        },
    };
    let bytes = compact::encode_delta(&all_unchanged).unwrap();
    assert_eq!(bytes[42], 0);
    assert_eq!(compact::decode_delta(&bytes).unwrap(), all_unchanged);
}

#[test]
fn identity_delta_matches_independent_literal_fixture_and_field_mask() {
    let delta = EgoDelta {
        ego_version: 0x0102_0304_0506_0708,
        previous_revision: 0x1112_1314_1516_1718,
        revision: 0x1112_1314_1516_1719,
        changes: EgoChanges::Identity {
            id: EgoIdentityId([0xA5; 16]),
            activate: true,
            name: FieldChange::Set("N".into()),
            text: FieldChange::Set("é".into()),
        },
    };
    let expected = [
        b"CVAEDL01".as_slice(),
        &[1],
        &0x0102_0304_0506_0708u64.to_le_bytes(),
        &0x1112_1314_1516_1718u64.to_le_bytes(),
        &[0xA5; 16],
        &[1, 3], // active and both fields changed
        &1u32.to_le_bytes(),
        b"N",
        &2u32.to_le_bytes(),
        "é".as_bytes(),
    ]
    .concat();
    let bytes = compact::encode_delta(&delta).unwrap();
    assert_eq!(bytes.len(), 54);
    assert_eq!(bytes, expected);
    assert_eq!(compact::decode_delta(&expected).unwrap(), delta);
}

#[test]
fn every_proper_prefix_of_delta_frames_is_rejected() {
    let samples = [
        EgoChanges::Identity {
            id: EgoIdentityId([1; 16]),
            activate: false,
            name: FieldChange::Set("name".into()),
            text: FieldChange::Set("body".into()),
        },
        EgoChanges::Anchor {
            id: EgoAnchorId([2; 16]),
            priority: FieldChange::Set(EgoAnchorPriority::High),
            text: FieldChange::Set("body".into()),
        },
        EgoChanges::Personality {
            source_memory_version: FieldChange::Set(8),
            text: FieldChange::Set("body".into()),
        },
        EgoChanges::Synthesis {
            source_memory_version: FieldChange::Set(9),
            text: FieldChange::Set("body".into()),
        },
    ];
    for changes in samples {
        let bytes = compact::encode_delta(&EgoDelta {
            ego_version: 1,
            previous_revision: 1,
            revision: 2,
            changes,
        })
        .unwrap();
        for end in 0..bytes.len() {
            invalid(compact::decode_delta(&bytes[..end]));
        }
    }
}

#[test]
fn rejects_unknown_versions_kinds_masks_trailing_bytes_and_invalid_envelopes() {
    let original = compact::encode_delta(&EgoDelta {
        ego_version: 1,
        previous_revision: 1,
        revision: 2,
        changes: EgoChanges::Anchor {
            id: EgoAnchorId([3; 16]),
            priority: FieldChange::Unchanged,
            text: FieldChange::Set("body".into()),
        },
    })
    .unwrap();
    let mut trailing = original.clone();
    trailing.push(0);
    invalid(compact::decode_delta(&trailing));
    let mut version = original.clone();
    version[7] = b'2';
    invalid(compact::decode_delta(&version));
    let mut kind = original.clone();
    kind[8] = 99;
    invalid(compact::decode_delta(&kind));
    let mut unknown_mask = original.clone();
    unknown_mask[41] |= 0x80;
    invalid(compact::decode_delta(&unknown_mask));

    let mut zero_ego_version = original.clone();
    zero_ego_version[9..17].copy_from_slice(&0u64.to_le_bytes());
    invalid(compact::decode_delta(&zero_ego_version));
    let mut zero_previous_revision = original.clone();
    zero_previous_revision[17..25].copy_from_slice(&0u64.to_le_bytes());
    invalid(compact::decode_delta(&zero_previous_revision));

    let overflow = EgoDelta {
        ego_version: 1,
        previous_revision: u64::MAX,
        revision: u64::MAX,
        changes: EgoChanges::Identity {
            id: EgoIdentityId([4; 16]),
            activate: false,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        },
    };
    invalid(compact::encode_delta(&overflow));
}

#[test]
fn raw_identical_publication_encodes_zero_field_mask() {
    let previous = identity(4, 7, EgoIdentityId([5; 16]), true, "same", "same");
    let next = identity(5, 8, EgoIdentityId([5; 16]), false, "same", "same");
    let delta = EgoDelta::between(&previous, &next).unwrap();
    assert_eq!(
        delta.changes,
        EgoChanges::Identity {
            id: EgoIdentityId([5; 16]),
            activate: false,
            name: FieldChange::Unchanged,
            text: FieldChange::Unchanged,
        }
    );
    let bytes = compact::encode_delta(&delta).unwrap();
    assert_eq!(bytes[42], 0);
    assert_eq!(compact::decode_delta(&bytes).unwrap(), delta);
}

#[test]
fn personality_and_synthesis_deltas_capture_metadata_only_decreasing_watermarks() {
    for (previous, next, expected) in [
        (
            EgoRecord::Personality {
                ego_version: 1,
                revision: 4,
                source_memory_version: 12,
                text: "same".into(),
            },
            EgoRecord::Personality {
                ego_version: 2,
                revision: 5,
                source_memory_version: 9,
                text: "same".into(),
            },
            EgoChanges::Personality {
                source_memory_version: FieldChange::Set(9),
                text: FieldChange::Unchanged,
            },
        ),
        (
            EgoRecord::Synthesis {
                ego_version: 3,
                revision: 8,
                source_memory_version: 22,
                text: "same".into(),
            },
            EgoRecord::Synthesis {
                ego_version: 4,
                revision: 9,
                source_memory_version: 3,
                text: "same".into(),
            },
            EgoChanges::Synthesis {
                source_memory_version: FieldChange::Set(3),
                text: FieldChange::Unchanged,
            },
        ),
    ] {
        let delta = EgoDelta::between(&previous, &next).unwrap();
        assert_eq!(delta.changes, expected);
        assert_eq!(
            compact::decode_delta(&compact::encode_delta(&delta).unwrap()).unwrap(),
            delta
        );
    }
}
