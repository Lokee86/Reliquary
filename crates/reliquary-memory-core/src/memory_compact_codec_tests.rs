use super::*;
use crate::memory_model::MemoryRecord;
use crate::{
    EpisodeId, MemoryBodyId, MemoryError, MemoryId, MemorySourceRef, MemoryTemporalInference,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};

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
        global_version: 4,
        memory_version: 2,
    }
}

fn successor(previous: &MemoryRecord) -> MemoryRecord {
    let mut next = previous.clone();
    next.revision += 1;
    next.global_version += 3;
    next.memory_version += 2;
    next.mutation_id = "next".into();
    next
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn creation_golden() -> Vec<u8> {
    let mut bytes = b"CVAMC001".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&[2; 32]);
    bytes.extend_from_slice(&4_u64.to_le_bytes());
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    push_string(&mut bytes, "m");
    for value in ["c", "t", "a", "ts", "s", "l"] {
        push_string(&mut bytes, value);
    }
    bytes.push(0); // archived
    for _ in 0..11 {
        bytes.push(0); // optional fields, through temporal inference
    }
    bytes.extend_from_slice(&(-7_i64).to_le_bytes());
    bytes.extend_from_slice(&9_i64.to_le_bytes());
    bytes
}

fn delta_golden() -> Vec<u8> {
    let mut bytes = b"CVAMD001".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // previous revision
    bytes.extend_from_slice(&2_u64.to_le_bytes()); // revision
    bytes.extend_from_slice(&7_u64.to_le_bytes()); // global version
    bytes.extend_from_slice(&4_u64.to_le_bytes()); // Memory version
    push_string(&mut bytes, "next");
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // category only
    push_string(&mut bytes, "changed");
    bytes
}

fn read_string_end(bytes: &[u8], offset: usize) -> usize {
    let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    offset + 4 + len
}

fn creation_fields_end(bytes: &[u8]) -> usize {
    let mut cursor = 88;
    // The mutation ID precedes the six string fields in the record payload.
    for _ in 0..7 {
        cursor = read_string_end(bytes, cursor);
    }
    cursor
}

fn delta_mask_offset(bytes: &[u8]) -> usize {
    read_string_end(bytes, 72)
}

fn delta_value_offset(bytes: &[u8]) -> usize {
    delta_mask_offset(bytes) + 8
}

#[test]
fn minimal_creation_matches_independently_built_golden_vector() {
    let creation = MemoryCreation::new(record()).unwrap();
    assert_eq!(encode_creation(&creation).unwrap(), creation_golden());
    assert_eq!(decode_creation(&creation_golden()).unwrap(), creation);
}

#[test]
fn minimal_delta_matches_independently_built_golden_vector() {
    let previous = record();
    let mut next = successor(&previous);
    next.category = "changed".into();
    let delta = MemoryDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 1);
    assert_eq!(encode_delta(&delta).unwrap(), delta_golden());
    assert_eq!(decode_delta(&delta_golden()).unwrap(), delta);
}

#[test]
fn every_truncated_prefix_and_trailing_byte_is_rejected() {
    for bytes in [creation_golden(), delta_golden()] {
        for end in 0..bytes.len() {
            assert!(
                decode_dispatch(&bytes[..end]).is_err(),
                "prefix length {end}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_dispatch(&trailing).is_err());
    }
}

fn decode_dispatch(bytes: &[u8]) -> Result<(), MemoryError> {
    if bytes.starts_with(b"CVAMC001") {
        decode_creation(bytes).map(|_| ())
    } else {
        decode_delta(bytes).map(|_| ())
    }
}

#[test]
fn bad_magic_is_rejected_for_both_frame_kinds() {
    let mut creation = creation_golden();
    creation[0] ^= 1;
    assert!(decode_creation(&creation).is_err());
    let mut delta = delta_golden();
    delta[0] ^= 1;
    assert!(decode_delta(&delta).is_err());
}

#[test]
fn creation_rejects_invalid_bool_optional_tags_utf8_and_lengths() {
    let golden = creation_golden();
    let fields_end = creation_fields_end(&golden);
    let mut bad_bool = golden.clone();
    bad_bool[fields_end] = 2;
    assert!(decode_creation(&bad_bool).is_err());

    let optional_start = fields_end + 1; // first optional follows archived
    let mut bad_id_tag = golden.clone();
    bad_id_tag[optional_start] = 2; // superseded_by
    assert!(decode_creation(&bad_id_tag).is_err());
    let mut bad_string_tag = golden.clone();
    bad_string_tag[optional_start + 2] = 2; // third optional is source_node_id
    assert!(decode_creation(&bad_string_tag).is_err());

    let mut invalid_utf8 = golden.clone();
    invalid_utf8[92] = 0xff; // mutation_id payload
    assert!(decode_creation(&invalid_utf8).is_err());
    let mut oversized = golden;
    oversized[88..92].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_creation(&oversized).is_err());

    let mut empty_mutation = creation_golden();
    empty_mutation[88..92].copy_from_slice(&0_u32.to_le_bytes());
    empty_mutation.drain(92..93);
    assert!(decode_creation(&empty_mutation).is_err());
}

#[test]
fn creation_rejects_zero_coordinates() {
    for offset in [72, 80] {
        let mut bytes = creation_golden();
        bytes[offset..offset + 8].fill(0);
        assert!(decode_creation(&bytes).is_err());
    }
}

#[test]
fn delta_rejects_bad_tags_unknown_masks_invalid_utf8_and_lengths() {
    let previous = record();
    let mut archived = successor(&previous);
    archived.archived = true;
    let mut bad_bool = encode_delta(&MemoryDelta::between(&previous, &archived).unwrap()).unwrap();
    let bool_value_offset = delta_value_offset(&bad_bool);
    bad_bool[bool_value_offset] = 2;
    assert!(decode_delta(&bad_bool).is_err());

    let mut with_optional = successor(&previous);
    with_optional.parent_id = Some(MemoryId([3; 32]));
    let mut bad_optional =
        encode_delta(&MemoryDelta::between(&previous, &with_optional).unwrap()).unwrap();
    let optional_value_offset = delta_value_offset(&bad_optional);
    bad_optional[optional_value_offset] = 2;
    assert!(decode_delta(&bad_optional).is_err());

    let mut unknown_mask = delta_golden();
    let mask_at = delta_mask_offset(&unknown_mask);
    unknown_mask[mask_at..mask_at + 8].copy_from_slice(&(1_u64 << 20).to_le_bytes());
    assert!(decode_delta(&unknown_mask).is_err());

    let mut invalid_utf8 = delta_golden();
    invalid_utf8[76] = 0xff; // mutation_id payload
    assert!(decode_delta(&invalid_utf8).is_err());
    let mut oversized = delta_golden();
    oversized[72..76].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_delta(&oversized).is_err());

    let mut empty_mutation = delta_golden();
    empty_mutation[72..76].copy_from_slice(&0_u32.to_le_bytes());
    empty_mutation.drain(76..80);
    assert!(decode_delta(&empty_mutation).is_err());
}

#[test]
fn delta_rejects_zero_or_inconsistent_coordinates() {
    let original = delta_golden();
    for (offset, value) in [
        (40, 0_u64), // previous revision
        (48, 0),     // revision
        (48, 3),     // must be previous + 1
        (56, 0),     // global version
        (64, 0),     // Memory version
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(decode_delta(&bytes).is_err(), "coordinate offset {offset}");
    }
}

#[test]
fn rich_optional_and_structured_values_round_trip_in_creation_and_delta() {
    let mut rich = record();
    rich.archived = true;
    rich.superseded_by = Some(MemoryId([3; 32]));
    rich.parent_id = Some(MemoryId([4; 32]));
    rich.source_node_id = Some("source-node".into());
    rich.content_source_conversation_id = Some("content-conversation".into());
    rich.content_source_node_id = Some("content-node".into());
    rich.grounding_source_conversation_id = Some("grounding-conversation".into());
    rich.grounding_source_node_id = Some("grounding-node".into());
    rich.source_episode_id = Some(EpisodeId([5; 32]));
    rich.source_time_ns = Some(-123456789);
    rich.source_ref = Some(MemorySourceRef {
        owner_id: "phy-owner".into(),
        principal_id: Some("principal".into()),
        source_episode_id: EpisodeId([6; 32]),
        source_node_id: "ref-node".into(),
        content_source_conversation_id: Some("ref-content-conversation".into()),
        content_source_node_id: Some("ref-content-node".into()),
        grounding_source_conversation_id: Some("ref-grounding-conversation".into()),
        grounding_source_node_id: Some("ref-grounding-node".into()),
    });
    rich.temporal_inference = Some(MemoryTemporalInference {
        body_id: rich.body_id,
        source_time_ns: Some(-42),
        inference: TemporalInference {
            model: "model".into(),
            contract_version: "contract-v1".into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 2,
                end_byte: 7,
                kind: TemporalIndicationKind::Relative,
                evidence: "last week".into(),
                canonical_expression: "P1W".into(),
            }],
        },
    });
    let creation = MemoryCreation::new(rich.clone()).unwrap();
    let encoded = encode_creation(&creation).unwrap();
    assert_eq!(decode_creation(&encoded).unwrap(), creation);

    let mut revised = successor(&record());
    let mut rich_delta_target = rich.clone();
    rich_delta_target.revision = revised.revision;
    rich_delta_target.global_version = revised.global_version;
    rich_delta_target.memory_version = revised.memory_version;
    rich_delta_target.mutation_id = revised.mutation_id.clone();
    revised = rich_delta_target.clone();
    revised.scope = "updated-scope".into();
    revised.parent_id = None;
    revised.source_time_ns = None;
    revised
        .temporal_inference
        .as_mut()
        .unwrap()
        .inference
        .resolutions[0]
        .evidence = "recently".into();
    let delta = MemoryDelta::between(&rich, &revised).unwrap();
    let encoded = encode_delta(&delta).unwrap();
    assert_eq!(decode_delta(&encoded).unwrap(), delta);
}
