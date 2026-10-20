use super::*;
use crate::compact_delta::FieldChange;
use crate::entity_compact_contract::{ENTITY_DELTA_KNOWN_MASK, EntityDescriptionProof};
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

#[test]
fn echo_payload_namespace_is_not_a_compact_entity_family() {
    assert!(!is_compact_record(b"CVAECHO1").unwrap());
}

fn record() -> EntityRecord {
    EntityRecord {
        id: EntityId([1; 32]),
        global_id: None,
        revision: 1,
        canonical_name: "name".into(),
        aliases: vec!["alias".into()],
        kind: "project".into(),
        summary: "summary".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: -7,
        updated_at_ns: 9,
        global_version: 4,
        entity_version: 2,
    }
}

fn next_record(previous: &EntityRecord) -> EntityRecord {
    let mut next = previous.clone();
    next.revision += 1;
    next.global_version = 1; // A late-bound older global ticket is valid.
    next.entity_version += 2; // Another Entity ID may own the intervening revision.
    next.mutation_id = format!("mutation-{}", next.revision);
    next
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn seal(mut bytes: Vec<u8>) -> Vec<u8> {
    let checksum = Sha256::digest(&bytes);
    bytes.extend_from_slice(&checksum);
    bytes
}

fn creation_golden() -> Vec<u8> {
    let mut bytes = b"CVAENC01".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.push(0); // no global UUID, as in an R1 record
    bytes.extend_from_slice(&4_u64.to_le_bytes());
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    push_string(&mut bytes, "mutation-1");
    push_string(&mut bytes, "name");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    push_string(&mut bytes, "alias");
    push_string(&mut bytes, "project");
    push_string(&mut bytes, "summary");
    bytes.extend_from_slice(&(-7_i64).to_le_bytes());
    bytes.extend_from_slice(&9_i64.to_le_bytes());
    bytes.push(0); // no pruned-description proof
    seal(bytes)
}

fn delta_golden() -> Vec<u8> {
    let mut bytes = b"CVAEND01".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // prior revision
    bytes.extend_from_slice(&2_u64.to_le_bytes()); // next revision
    bytes.extend_from_slice(&1_u64.to_le_bytes()); // late global ticket
    bytes.extend_from_slice(&4_u64.to_le_bytes()); // local Entity version
    push_string(&mut bytes, "mutation-2");
    bytes.extend_from_slice(&(1_u64 << 1).to_le_bytes()); // canonical name
    push_string(&mut bytes, "renamed");
    bytes.push(0); // no pruned-description proof
    seal(bytes)
}

fn read_u32_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn string_end(bytes: &[u8], offset: usize) -> usize {
    offset + 4 + read_u32_at(bytes, offset)
}

fn creation_proof_flag_offset(bytes: &[u8]) -> usize {
    let mut cursor = 57; // mutation string
    cursor = string_end(bytes, cursor);
    cursor = string_end(bytes, cursor); // canonical name
    let aliases = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
    cursor += 2;
    for _ in 0..aliases {
        cursor = string_end(bytes, cursor);
    }
    cursor = string_end(bytes, cursor); // kind
    cursor = string_end(bytes, cursor); // summary
    cursor + 16 // signed creation and update times
}

fn delta_mask_offset(bytes: &[u8]) -> usize {
    string_end(bytes, 72)
}

fn delta_proof_flag_offset(bytes: &[u8]) -> usize {
    let mask = delta_mask_offset(bytes);
    let bits = u64::from_le_bytes(bytes[mask..mask + 8].try_into().unwrap());
    let mut cursor = mask + 8;
    for bit in 0..6 {
        if bits & (1 << bit) == 0 {
            continue;
        }
        cursor = match bit {
            0 => cursor + 1 + usize::from(bytes[cursor] == 1) * 16,
            1 | 3 | 4 => string_end(bytes, cursor),
            2 => {
                let count = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
                cursor += 2;
                for _ in 0..count {
                    cursor = string_end(bytes, cursor);
                }
                cursor
            }
            5 => cursor + 8,
            _ => unreachable!(),
        };
    }
    cursor
}

fn decode_frame(bytes: &[u8], creation: bool) -> Result<(), EntityError> {
    if creation {
        decode_creation(bytes).map(|_| ())
    } else {
        decode_delta(bytes).map(|_| ())
    }
}

fn refresh_checksum(bytes: &mut Vec<u8>) {
    let checksum_at = bytes.len() - 32;
    let checksum = Sha256::digest(&bytes[..checksum_at]);
    bytes[checksum_at..].copy_from_slice(&checksum);
}

#[test]
fn minimal_creation_matches_independent_golden_vector() {
    let creation = EntityCreation::new(record()).unwrap();
    assert_eq!(encode_creation(&creation).unwrap(), creation_golden());
    assert_eq!(decode_creation(&creation_golden()).unwrap(), creation);
}

#[test]
fn minimal_delta_matches_independent_golden_vector() {
    let previous = record();
    let mut next = next_record(&previous);
    next.canonical_name = "renamed".into();
    let delta = EntityDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 1 << 1);
    assert_eq!(encode_delta(&delta).unwrap(), delta_golden());
    assert_eq!(decode_delta(&delta_golden()).unwrap(), delta);
}

#[test]
fn all_creation_and_delta_fields_round_trip_with_identity_transitions() {
    let mut rich = record();
    rich.global_id = Some(EntityGlobalId([0xA1; 16]));
    rich.canonical_name = "A richer Entity".into();
    rich.aliases = vec!["Alpha".into(), "Zeta".into()];
    rich.kind = "service".into();
    rich.summary = "Structured description".into();
    rich.created_at_ns = -100;
    rich.updated_at_ns = -50;
    let creation = EntityCreation::new(rich.clone()).unwrap();
    assert_eq!(
        decode_creation(&encode_creation(&creation).unwrap()).unwrap(),
        creation
    );

    let mut revised = next_record(&rich);
    revised.global_id = Some(EntityGlobalId([0xB2; 16]));
    revised.canonical_name = "Another name".into();
    revised.aliases = vec!["Beta".into(), "Omega".into()];
    revised.kind = "organization".into();
    revised.summary = "Replaced summary".into();
    revised.updated_at_ns = -75; // backward, while still >= creation time
    let delta = EntityDelta::between(&rich, &revised).unwrap();
    assert_eq!(delta.fields.mask(), ENTITY_DELTA_KNOWN_MASK);
    assert_eq!(decode_delta(&encode_delta(&delta).unwrap()).unwrap(), delta);

    // Each Option state is a distinct field value: None -> Some(A) -> Some(B) -> None.
    let mut no_uuid = record();
    no_uuid.global_version = 2;
    let mut with_a = next_record(&no_uuid);
    with_a.global_id = Some(EntityGlobalId([0xA1; 16]));
    assert_eq!(
        EntityDelta::between(&no_uuid, &with_a)
            .unwrap()
            .fields
            .mask(),
        1
    );
    let mut with_b = next_record(&with_a);
    with_b.global_id = Some(EntityGlobalId([0xB2; 16]));
    with_b.global_version = 3;
    assert_eq!(
        EntityDelta::between(&with_a, &with_b)
            .unwrap()
            .fields
            .mask(),
        1
    );
    let mut cleared = next_record(&with_b);
    cleared.global_id = None;
    cleared.global_version = 4;
    assert_eq!(
        EntityDelta::between(&with_b, &cleared)
            .unwrap()
            .fields
            .mask(),
        1
    );
}

#[test]
fn every_delta_mask_round_trips_with_the_selected_typed_values() {
    let previous = record();
    for mask in 0..=ENTITY_DELTA_KNOWN_MASK {
        let mut next = next_record(&previous);
        if mask & (1 << 0) != 0 {
            next.global_id = Some(EntityGlobalId([0x31; 16]));
        }
        if mask & (1 << 1) != 0 {
            next.canonical_name = "renamed".into();
        }
        if mask & (1 << 2) != 0 {
            next.aliases = vec!["changed".into()];
        }
        if mask & (1 << 3) != 0 {
            next.kind = "service".into();
        }
        if mask & (1 << 4) != 0 {
            next.summary = "changed summary".into();
        }
        if mask & (1 << 5) != 0 {
            next.updated_at_ns += 1;
        }
        let delta = EntityDelta::between(&previous, &next).unwrap();
        assert_eq!(delta.fields.mask(), mask);
        let encoded = encode_delta(&delta).unwrap();
        assert_eq!(decode_delta(&encoded).unwrap(), delta, "mask {mask:#08b}");
    }
}

#[test]
fn zero_mask_fresh_mutation_round_trips() {
    let previous = record();
    let mut next = next_record(&previous);
    next.canonical_name = previous.canonical_name.clone();
    next.aliases = previous.aliases.clone();
    next.kind = previous.kind.clone();
    next.summary = previous.summary.clone();
    next.updated_at_ns = previous.updated_at_ns;
    assert_eq!(
        EntityDelta::between(&previous, &next)
            .unwrap()
            .fields
            .mask(),
        0
    );
    let delta = EntityDelta::between(&previous, &next).unwrap();
    assert_eq!(decode_delta(&encode_delta(&delta).unwrap()).unwrap(), delta);
}

#[test]
fn pruned_description_proof_is_explicit_and_not_inferred_from_placeholder_text() {
    let proof = EntityDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let mut pruned_record = record();
    pruned_record.canonical_name = "[pruned]".into();
    pruned_record.aliases.clear();
    pruned_record.summary.clear();

    let authored_placeholder = EntityCreation::new(pruned_record.clone()).unwrap();
    let authored_bytes = encode_creation(&authored_placeholder).unwrap();
    assert_eq!(
        decode_creation(&authored_bytes).unwrap(),
        authored_placeholder
    );
    assert_eq!(authored_placeholder.description_proof, None);
    assert_eq!(
        authored_bytes[creation_proof_flag_offset(&authored_bytes)],
        0
    );

    let pruned = EntityCreation::new_pruned(pruned_record.clone(), proof.clone()).unwrap();
    let pruned_bytes = encode_creation(&pruned).unwrap();
    assert_eq!(decode_creation(&pruned_bytes).unwrap(), pruned);
    assert_eq!(pruned_bytes[creation_proof_flag_offset(&pruned_bytes)], 1);

    let previous = record();
    let mut pruned_revision = pruned_record.clone();
    pruned_revision.revision = 2;
    pruned_revision.global_version = 1;
    pruned_revision.entity_version = 4;
    pruned_revision.mutation_id = "mutation-pruned".into();
    let delta = EntityDelta::between_pruned(&previous, &pruned_revision, proof.clone()).unwrap();
    let delta_bytes = encode_delta(&delta).unwrap();
    assert_eq!(decode_delta(&delta_bytes).unwrap(), delta);
    assert_eq!(delta_bytes[delta_proof_flag_offset(&delta_bytes)], 1);
}

#[test]
fn checksum_truncation_trailing_bytes_and_unknown_versions_fail_closed() {
    let creation = creation_golden();
    let delta = delta_golden();
    for (bytes, is_creation) in [(creation, true), (delta, false)] {
        for end in 0..bytes.len() {
            assert!(
                decode_frame(&bytes[..end], is_creation).is_err(),
                "prefix {end}"
            );
        }
        let mut trailing = bytes.clone();
        let checksum_at = trailing.len() - 32;
        trailing.insert(checksum_at, 0xEE);
        refresh_checksum(&mut trailing);
        assert!(is_compact_record(&trailing).is_err());
    }

    let mut bad_creation_version = creation_golden();
    bad_creation_version[7] = b'2';
    refresh_checksum(&mut bad_creation_version);
    assert!(is_compact_record(&bad_creation_version).is_err());
    let mut bad_delta_version = delta_golden();
    bad_delta_version[7] = b'2';
    refresh_checksum(&mut bad_delta_version);
    assert!(is_compact_record(&bad_delta_version).is_err());

    let mut corrupt_checksum = creation_golden();
    corrupt_checksum[20] ^= 1;
    assert!(decode_creation(&corrupt_checksum).is_err());
}

#[test]
fn malformed_tags_masks_utf8_and_lengths_fail_closed() {
    let mut bad_global_tag = creation_golden();
    bad_global_tag[40] = 2;
    refresh_checksum(&mut bad_global_tag);
    assert!(decode_creation(&bad_global_tag).is_err());

    let proof_flag = creation_proof_flag_offset(&creation_golden());
    let mut bad_proof_tag = creation_golden();
    bad_proof_tag[proof_flag] = 2;
    refresh_checksum(&mut bad_proof_tag);
    assert!(decode_creation(&bad_proof_tag).is_err());

    let mut invalid_utf8 = creation_golden();
    invalid_utf8[61] = 0xFF; // first mutation-ID byte
    refresh_checksum(&mut invalid_utf8);
    assert!(decode_creation(&invalid_utf8).is_err());

    let mut oversized_string = creation_golden();
    oversized_string[57..61].copy_from_slice(&u32::MAX.to_le_bytes());
    refresh_checksum(&mut oversized_string);
    assert!(decode_creation(&oversized_string).is_err());

    let mut too_many_aliases = creation_golden();
    let canonical_start = string_end(&too_many_aliases, 57);
    let aliases_count = string_end(&too_many_aliases, canonical_start);
    too_many_aliases[aliases_count..aliases_count + 2]
        .copy_from_slice(&((crate::MAX_ENTITY_ALIASES + 1) as u16).to_le_bytes());
    refresh_checksum(&mut too_many_aliases);
    assert!(decode_creation(&too_many_aliases).is_err());

    let mut unknown_mask = delta_golden();
    let mask = delta_mask_offset(&unknown_mask);
    unknown_mask[mask..mask + 8].copy_from_slice(&(1_u64 << 6).to_le_bytes());
    refresh_checksum(&mut unknown_mask);
    assert!(decode_delta(&unknown_mask).is_err());

    let mut changed_uuid = next_record(&record());
    changed_uuid.global_id = Some(EntityGlobalId([0xEE; 16]));
    let mut bad_delta_global_tag =
        encode_delta(&EntityDelta::between(&record(), &changed_uuid).unwrap()).unwrap();
    let global_tag = delta_mask_offset(&bad_delta_global_tag) + 8;
    bad_delta_global_tag[global_tag] = 2;
    refresh_checksum(&mut bad_delta_global_tag);
    assert!(decode_delta(&bad_delta_global_tag).is_err());

    let mut delta_invalid_utf8 = delta_golden();
    delta_invalid_utf8[76] = 0xFF;
    refresh_checksum(&mut delta_invalid_utf8);
    assert!(decode_delta(&delta_invalid_utf8).is_err());

    let mut delta_oversized_string = delta_golden();
    delta_oversized_string[72..76].copy_from_slice(&u32::MAX.to_le_bytes());
    refresh_checksum(&mut delta_oversized_string);
    assert!(decode_delta(&delta_oversized_string).is_err());
}

#[test]
fn malformed_delta_coordinates_and_empty_mutation_fail_closed() {
    let original = delta_golden();
    for (offset, value) in [
        (40, 0_u64), // previous revision
        (48, 0),     // revision
        (48, 3),     // must be previous + 1
        (56, 0),     // global version
        (64, 0),     // entity version
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        refresh_checksum(&mut bytes);
        assert!(decode_delta(&bytes).is_err(), "coordinate {offset}");
    }

    let mut empty_mutation = original;
    empty_mutation[72..76].copy_from_slice(&0_u32.to_le_bytes());
    empty_mutation.drain(76..86); // remove mutation-2
    refresh_checksum(&mut empty_mutation);
    assert!(decode_delta(&empty_mutation).is_err());
}

#[test]
fn malformed_creation_coordinates_fail_closed() {
    for offset in [41, 49] {
        let mut bytes = creation_golden();
        bytes[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes());
        refresh_checksum(&mut bytes);
        assert!(decode_creation(&bytes).is_err(), "coordinate {offset}");
    }
}

#[test]
fn encoder_rejects_invalid_owner_records_and_masks() {
    let mut invalid_creation = record();
    invalid_creation.global_version = 0;
    assert!(EntityCreation::new(invalid_creation).is_err());

    let previous = record();
    let mut next = next_record(&previous);
    next.mutation_id.clear();
    assert!(EntityDelta::between(&previous, &next).is_err());

    let mut invalid = EntityDelta::between(&previous, &next_record(&previous)).unwrap();
    invalid.previous_revision = 0;
    assert!(encode_delta(&invalid).is_err());

    let mut reserved_mask = EntityDelta::between(&previous, &next_record(&previous)).unwrap();
    reserved_mask.fields.global_id = FieldChange::Set(Some(EntityGlobalId([9; 16])));
    // The owner field structure itself can only emit known bits; direct mask
    // validation protects future construction paths.
    assert!(EntityDelta::validate_mask(reserved_mask.fields.mask()).is_ok());
    assert!(EntityDelta::validate_mask(1 << 63).is_err());
}
