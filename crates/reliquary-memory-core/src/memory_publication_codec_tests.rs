use super::{MemoryPublication, decode, encode};
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta};
use crate::memory_model::MemoryRecord;
use crate::{MemoryBodyId, MemoryId};

fn record() -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([1; 32]),
        revision: 1,
        body_id: MemoryBodyId([2; 32]),
        category: "c".into(),
        memory_type: "t".into(),
        authority_kind: "a".into(),
        temporal_status: "ts".into(),
        scope: "s".into(),
        lifecycle_state: "l".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        source_ref: None,
        temporal_inference: None,
        mutation_id: "m".into(),
        created_at_ns: -7,
        updated_at_ns: 9,
        global_version: 31,
        memory_version: 2,
    }
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

// Independent fixed-layout byte fixture. In particular, global order and the
// implicit revision-one coordinate are omitted from this owner-local body.
fn creation_golden() -> Vec<u8> {
    let mut bytes = b"CVAMC002".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&[2; 32]);
    bytes.extend_from_slice(&2_u64.to_le_bytes()); // Memory version
    push_string(&mut bytes, "m");
    for value in ["c", "t", "a", "ts", "s", "l"] {
        push_string(&mut bytes, value);
    }
    bytes.push(0); // archived
    bytes.extend_from_slice(&[0; 11]); // all optional creation values
    bytes.extend_from_slice(&(-7_i64).to_le_bytes());
    bytes.extend_from_slice(&9_i64.to_le_bytes());
    bytes
}

fn delta_golden() -> Vec<u8> {
    let mut bytes = b"CVAMD002".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // previous revision
    bytes.extend_from_slice(&2_u64.to_le_bytes()); // revision
    bytes.extend_from_slice(&4_u64.to_le_bytes()); // Memory version
    push_string(&mut bytes, "next");
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // category bit
    push_string(&mut bytes, "changed");
    bytes
}

fn tombstone_golden() -> Vec<u8> {
    let mut bytes = b"CVAMT002".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&3_u64.to_le_bytes());
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes
}

fn decode_as(bytes: &[u8], global: u64) -> Result<Option<MemoryPublication>, crate::MemoryError> {
    decode(bytes, global)
}

#[test]
fn all_publication_kinds_match_independent_golden_bytes_and_decode_context() {
    let creation = MemoryPublication::Creation(MemoryCreation::new(record()).unwrap());
    let mut next = record();
    next.revision = 2;
    next.global_version = 32;
    next.memory_version = 4;
    next.mutation_id = "next".into();
    next.category = "changed".into();
    let delta = MemoryPublication::Delta(MemoryDelta::between(&record(), &next).unwrap());
    let tombstone = MemoryPublication::Tombstone {
        id: MemoryId([1; 32]),
        revision: 3,
        memory_version: 8,
    };

    let creation_bytes = creation_golden();
    let delta_bytes = delta_golden();
    let terminal_bytes = tombstone_golden();
    assert_eq!(encode(&creation).unwrap(), creation_bytes);
    assert_eq!(encode(&delta).unwrap(), delta_bytes);
    assert_eq!(encode(&tombstone).unwrap(), terminal_bytes);
    assert_eq!(
        creation_bytes.len(),
        8 + 32 + 32 + 8 + (4 + 1) + 31 + 1 + 11 + 16
    );
    assert_eq!(delta_bytes.len(), 8 + 32 + 8 * 3 + 4 + 4 + 8 + 4 + 7);
    assert_eq!(terminal_bytes.len(), 8 + 32 + 8 + 8);
    assert_eq!(decode_as(&creation_bytes, 31).unwrap(), Some(creation));
    assert_eq!(decode_as(&delta_bytes, 32).unwrap(), Some(delta));
    assert_eq!(decode_as(&terminal_bytes, 33).unwrap(), Some(tombstone));
    let Some(MemoryPublication::Creation(decoded_creation)) = decode(&creation_bytes, 31).unwrap()
    else {
        panic!("creation frame expected")
    };
    assert_eq!(decoded_creation.record.revision, 1);
}

#[test]
fn global_context_is_supplied_externally_and_never_changes_encoded_bytes() {
    let publication = MemoryPublication::Creation(MemoryCreation::new(record()).unwrap());
    let bytes = encode(&publication).unwrap();
    let mut another_global = record();
    another_global.global_version = 987;
    assert_eq!(
        encode(&MemoryPublication::Creation(
            MemoryCreation::new(another_global).unwrap()
        ))
        .unwrap(),
        bytes
    );
    let Some(MemoryPublication::Creation(decoded)) = decode(&bytes, 987).unwrap() else {
        panic!("creation frame expected")
    };
    assert_eq!(decoded.record.global_version, 987);
}

#[test]
fn strings_preserve_utf8_whitespace_and_empty_delta_preserves_unchanged_fields() {
    let mut spaced = record();
    spaced.global_version = 13;
    spaced.category = " \t猫\n  ".into();
    spaced.mutation_id = "  mut\té".into();
    let creation = MemoryPublication::Creation(MemoryCreation::new(spaced.clone()).unwrap());
    assert_eq!(
        decode(&encode(&creation).unwrap(), 13).unwrap(),
        Some(creation)
    );

    let mut next = record();
    next.revision = 2;
    next.global_version = 14;
    next.memory_version = 3;
    next.mutation_id = "next".into();
    let empty_delta = MemoryDelta::between(&record(), &next).unwrap();
    assert_eq!(empty_delta.fields.mask(), 0);
    let publication = MemoryPublication::Delta(empty_delta);
    let bytes = encode(&publication).unwrap();
    assert_eq!(
        u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap()),
        0
    );
    assert_eq!(decode(&bytes, 14).unwrap(), Some(publication));
}

#[test]
fn optional_clear_is_encoded_as_a_set_clear_value() {
    let mut before = record();
    before.source_node_id = Some("present".into());
    let mut after = before.clone();
    after.revision = 2;
    after.global_version = 50;
    after.memory_version = 3;
    after.mutation_id = "clear".into();
    after.source_node_id = None;
    let delta = MemoryDelta::between(&before, &after).unwrap();
    assert_eq!(delta.fields.mask(), 1 << 9);
    let bytes = encode(&MemoryPublication::Delta(delta.clone())).unwrap();
    // The clear remains present in the selected delta field as option tag 0.
    assert_eq!(*bytes.last().unwrap(), 0);
    assert_eq!(
        decode(&bytes, 50).unwrap(),
        Some(MemoryPublication::Delta(delta))
    );
}

#[test]
fn recognized_frames_reject_every_proper_prefix_trailing_bytes_and_unknown_versions() {
    for bytes in [creation_golden(), delta_golden(), tombstone_golden()] {
        // Before the five-byte family discriminator, a payload is unrelated.
        for end in 0..5 {
            assert_eq!(decode(&bytes[..end], 1).unwrap(), None, "prefix {end}");
        }
        for end in 5..bytes.len() {
            assert!(decode(&bytes[..end], 1).is_err(), "prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing, 1).is_err());
        assert!(decode(&bytes, 0).is_err(), "zero global context");
    }
    for mut bytes in [creation_golden(), delta_golden(), tombstone_golden()] {
        bytes[7] = b'9';
        assert!(decode(&bytes, 1).is_err());
    }
    assert_eq!(decode(b"unrelated", 1).unwrap(), None);
}

#[test]
fn malformed_coordinates_flags_masks_and_utf8_are_rejected() {
    let creation = creation_golden();
    for (offset, value) in [(72, 0_u64)] {
        let mut bad = creation.clone();
        bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&bad, 1).is_err());
    }
    let mut unrelated = creation.clone();
    unrelated[0] = b'X';
    assert_eq!(decode(&unrelated, 0).unwrap(), None);

    // For this fixture the archived bool follows the six required strings.
    let archived_at = 8 + 32 + 32 + 8 + 5 + 31;
    let mut bad_bool = creation.clone();
    bad_bool[archived_at] = 2;
    assert!(decode(&bad_bool, 1).is_err());
    let mut bad_optional = creation.clone();
    bad_optional[archived_at + 2] = 2;
    assert!(decode(&bad_optional, 1).is_err());
    let mut bad_utf8 = creation;
    bad_utf8[8 + 32 + 32 + 8 + 4] = 0xff;
    assert!(decode(&bad_utf8, 1).is_err());

    let delta = delta_golden();
    for (offset, value) in [(40, 0_u64), (48, 3_u64), (56, 0_u64)] {
        let mut bad = delta.clone();
        bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&bad, 7).is_err(), "coordinate {offset}");
    }
    let mask_offset = 8 + 32 + 24 + 4 + 4;
    let mut unknown_mask = delta.clone();
    unknown_mask[mask_offset..mask_offset + 8].copy_from_slice(&(1_u64 << 40).to_le_bytes());
    assert!(decode(&unknown_mask, 7).is_err());
    let mut bad_delta_utf8 = delta;
    bad_delta_utf8[8 + 32 + 24 + 4] = 0xff;
    assert!(decode(&bad_delta_utf8, 7).is_err());

    let terminal = tombstone_golden();
    for offset in [40, 48] {
        let mut bad = terminal.clone();
        bad[offset..offset + 8].fill(0);
        assert!(decode(&bad, 9).is_err());
    }
}

#[test]
fn encode_failures_leave_creation_and_delta_contract_values_unchanged() {
    let mut invalid_record = record();
    invalid_record.memory_version = 0;
    let invalid_creation = MemoryPublication::Creation(MemoryCreation {
        record: invalid_record,
    });
    let before = invalid_creation.clone();
    assert!(encode(&invalid_creation).is_err());
    assert_eq!(invalid_creation, before);

    let mut next = record();
    next.revision = 2;
    next.global_version = 32;
    next.memory_version = 4;
    next.mutation_id = "next".into();
    let mut invalid_delta = MemoryDelta::between(&record(), &next).unwrap();
    invalid_delta.revision = 4;
    let invalid_delta = MemoryPublication::Delta(invalid_delta);
    let before = invalid_delta.clone();
    assert!(encode(&invalid_delta).is_err());
    assert_eq!(invalid_delta, before);
}
