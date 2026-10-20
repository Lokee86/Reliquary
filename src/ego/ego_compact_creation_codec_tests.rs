use crate::ego_codec::{EgoRecord, compact};
use crate::ego_compact_contract::{EgoCreation, EgoTerminal};
use crate::{EgoAnchorId, EgoAnchorPriority, EgoError, EgoIdentityId};

fn identity(version: u64, id: EgoIdentityId, activate: bool, name: &str, text: &str) -> EgoRecord {
    EgoRecord::Identity {
        ego_version: version,
        id,
        revision: 1,
        deleted: false,
        activate,
        name: name.into(),
        text: text.into(),
    }
}

fn anchor(version: u64, id: EgoAnchorId, priority: EgoAnchorPriority, text: &str) -> EgoRecord {
    EgoRecord::Anchor {
        ego_version: version,
        id,
        revision: 1,
        deleted: false,
        priority,
        text: text.into(),
    }
}

fn creation(record: EgoRecord) -> EgoCreation {
    EgoCreation::new(record).unwrap()
}

fn invalid<T>(result: Result<T, EgoError>) {
    assert!(matches!(result, Err(EgoError::InvalidRecord(_))));
}

#[test]
fn creation_roundtrips_all_four_kinds_and_preserves_utf8_whitespace() {
    let id = EgoIdentityId([0x11; 16]);
    let aid = EgoAnchorId([0x22; 16]);
    let records = [
        identity(7, id, true, "  名称\n", " \t自我\n "),
        anchor(8, aid, EgoAnchorPriority::Low, " \t記憶\n "),
        EgoRecord::Personality {
            ego_version: 9,
            revision: 1,
            source_memory_version: 0x0102,
            text: "  élan\n ".into(),
        },
        EgoRecord::Synthesis {
            ego_version: 10,
            revision: 1,
            source_memory_version: 0x0304,
            text: " \t合成 ".into(),
        },
    ];

    for record in records {
        let expected = creation(record);
        let encoded = compact::encode_creation(&expected).unwrap();
        assert_eq!(compact::decode_creation(&encoded).unwrap(), expected);
    }
}

#[test]
fn creation_has_independent_literal_identity_fixture_and_omits_revision() {
    let expected = EgoCreation::new(identity(
        0x0102_0304_0506_0708,
        EgoIdentityId([0xA5; 16]),
        true,
        "N",
        "é",
    ))
    .unwrap();
    let bytes = compact::encode_creation(&expected).unwrap();

    // 8 magic + 1 kind + 8 Ego version + 16 ID + 1 activation +
    // (4 + 1) name + (4 + 2) UTF-8 text = 45 bytes. Revision is implicit.
    assert_eq!(bytes.len(), 45);
    assert_eq!(
        bytes,
        [
            b"CVAECR01".as_slice(),
            &[1],
            &0x0102_0304_0506_0708u64.to_le_bytes(),
            &[0xA5; 16],
            &[1],
            &1u32.to_le_bytes(),
            b"N",
            &2u32.to_le_bytes(),
            "é".as_bytes(),
        ]
        .concat()
    );
    assert_eq!(compact::decode_creation(&bytes).unwrap(), expected);
}

#[test]
fn legacy_identity_creation_normalizes_to_default_identity() {
    let legacy = EgoRecord::LegacyIdentity {
        ego_version: 3,
        revision: 1,
        text: " legacy ".into(),
    };
    let normalized = identity(
        3,
        crate::ego_store::LEGACY_IDENTITY_ID,
        true,
        "Default",
        " legacy ",
    );
    let creation = EgoCreation::new(legacy).unwrap();
    assert_eq!(creation.record, normalized);
    assert_eq!(
        compact::decode_creation(&compact::encode_creation(&creation).unwrap()).unwrap(),
        EgoCreation::new(normalized).unwrap()
    );
}

#[test]
fn terminal_roundtrips_both_kinds_and_retains_empty_payload_shape() {
    let id = EgoIdentityId([0x31; 16]);
    let aid = EgoAnchorId([0x42; 16]);
    let terminals = [
        EgoRecord::Identity {
            ego_version: 11,
            id,
            revision: 2,
            deleted: true,
            activate: false,
            name: String::new(),
            text: String::new(),
        },
        EgoRecord::Anchor {
            ego_version: 12,
            id: aid,
            revision: 9,
            deleted: true,
            priority: EgoAnchorPriority::High,
            text: String::new(),
        },
    ];

    for record in terminals {
        let expected = EgoTerminal::new(record).unwrap();
        let bytes = compact::encode_terminal(&expected).unwrap();
        assert_eq!(compact::decode_terminal(&bytes).unwrap(), expected);
        match expected.record {
            EgoRecord::Identity { .. } => {
                // 8 magic + 1 kind + 8 Ego version + 16 ID + 8 revision.
                // Both terminal strings and the activation byte are omitted.
                assert_eq!(bytes.len(), 41);
            }
            EgoRecord::Anchor { .. } => {
                // Identity's 41 bytes plus the exact one-byte priority.
                assert_eq!(bytes.len(), 42);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn every_proper_prefix_of_creation_and_terminal_frames_is_rejected() {
    let c = compact::encode_creation(&creation(identity(
        1,
        EgoIdentityId([1; 16]),
        false,
        "name",
        "body",
    )))
    .unwrap();
    let t = compact::encode_terminal(
        &EgoTerminal::new(EgoRecord::Anchor {
            ego_version: 2,
            id: EgoAnchorId([2; 16]),
            revision: 3,
            deleted: true,
            priority: EgoAnchorPriority::Normal,
            text: String::new(),
        })
        .unwrap(),
    )
    .unwrap();

    for end in 0..c.len() {
        invalid(compact::decode_creation(&c[..end]));
    }
    for end in 0..t.len() {
        invalid(compact::decode_terminal(&t[..end]));
    }
}

#[test]
fn rejects_trailing_bytes_unknown_versions_and_kinds() {
    let c = compact::encode_creation(&creation(identity(
        1,
        EgoIdentityId([3; 16]),
        true,
        "name",
        "body",
    )))
    .unwrap();
    let t = compact::encode_terminal(
        &EgoTerminal::new(EgoRecord::Identity {
            ego_version: 1,
            id: EgoIdentityId([4; 16]),
            revision: 1,
            deleted: true,
            activate: false,
            name: String::new(),
            text: String::new(),
        })
        .unwrap(),
    )
    .unwrap();

    let mut trailing = c.clone();
    trailing.push(0);
    invalid(compact::decode_creation(&trailing));
    let mut trailing_terminal = t.clone();
    trailing_terminal.push(0);
    invalid(compact::decode_terminal(&trailing_terminal));

    let mut bad_magic = c.clone();
    bad_magic[7] = b'2';
    invalid(compact::decode_creation(&bad_magic));
    let mut unknown_creation_kind = c;
    unknown_creation_kind[8] = 99;
    invalid(compact::decode_creation(&unknown_creation_kind));
    let mut unknown_terminal_kind = t;
    unknown_terminal_kind[8] = 99;
    invalid(compact::decode_terminal(&unknown_terminal_kind));
}

#[test]
fn rejects_invalid_flags_priorities_zero_clocks_and_terminal_shapes() {
    let mut c = compact::encode_creation(&creation(identity(
        1,
        EgoIdentityId([5; 16]),
        false,
        "name",
        "body",
    )))
    .unwrap();
    c[33] = 2; // Identity activation must be 0 or 1.
    invalid(compact::decode_creation(&c));

    let mut a = compact::encode_creation(&creation(anchor(
        1,
        EgoAnchorId([6; 16]),
        EgoAnchorPriority::Normal,
        "body",
    )))
    .unwrap();
    a[33] = 0; // Priority values are High=1, Normal=2, Low=3.
    invalid(compact::decode_creation(&a));

    let zero_version = compact::encode_creation(&creation(identity(
        1,
        EgoIdentityId([7; 16]),
        false,
        "name",
        "body",
    )))
    .unwrap();
    let mut zero_version = zero_version;
    zero_version[9..17].copy_from_slice(&0u64.to_le_bytes());
    invalid(compact::decode_creation(&zero_version));

    let mut zero_terminal_revision = compact::encode_terminal(
        &EgoTerminal::new(EgoRecord::Identity {
            ego_version: 2,
            id: EgoIdentityId([8; 16]),
            revision: 1,
            deleted: true,
            activate: false,
            name: String::new(),
            text: String::new(),
        })
        .unwrap(),
    )
    .unwrap();
    zero_terminal_revision[33..41].copy_from_slice(&0u64.to_le_bytes());
    invalid(compact::decode_terminal(&zero_terminal_revision));

    invalid(EgoCreation::new(identity(
        0,
        EgoIdentityId([9; 16]),
        false,
        "n",
        "t",
    )));
    invalid(EgoCreation::new(EgoRecord::Identity {
        ego_version: 1,
        id: EgoIdentityId([9; 16]),
        revision: 0,
        deleted: false,
        activate: false,
        name: "n".into(),
        text: "t".into(),
    }));
    invalid(EgoTerminal::new(EgoRecord::Identity {
        ego_version: 1,
        id: EgoIdentityId([10; 16]),
        revision: 1,
        deleted: true,
        activate: true,
        name: String::new(),
        text: String::new(),
    }));
    invalid(EgoTerminal::new(EgoRecord::Anchor {
        ego_version: 1,
        id: EgoAnchorId([11; 16]),
        revision: 1,
        deleted: true,
        priority: EgoAnchorPriority::Low,
        text: "body".into(),
    }));
    invalid(EgoTerminal::new(identity(
        1,
        EgoIdentityId([12; 16]),
        false,
        "n",
        "t",
    )));
}
