use crate::entity_compact_codec::publication::{EntityPublication, decode, encode};
use crate::entity_compact_contract::{
    ENTITY_DELTA_KNOWN_MASK, EntityCreation, EntityDelta, EntityDescriptionProof,
};
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

fn record() -> EntityRecord {
    EntityRecord {
        id: EntityId([1; 32]),
        global_id: Some(EntityGlobalId([2; 16])),
        revision: 1,
        canonical_name: "name".into(),
        aliases: vec!["alias".into()],
        kind: "project".into(),
        summary: "summary".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: -7,
        updated_at_ns: 9,
        global_version: 41,
        entity_version: 3,
    }
}

fn next_record(previous: &EntityRecord) -> EntityRecord {
    let mut next = previous.clone();
    next.revision = previous.revision + 1;
    next.global_version = 42;
    next.entity_version = 5;
    next.mutation_id = "mutation-2".into();
    next.canonical_name = "renamed".into();
    next
}

fn creation() -> EntityCreation {
    EntityCreation::new(record()).unwrap()
}

fn delta() -> EntityDelta {
    EntityDelta::between(&record(), &next_record(&record())).unwrap()
}

fn legacy_creation_body() -> Vec<u8> {
    crate::entity_compact_codec::encode_creation(&creation()).unwrap()
}

fn legacy_delta_body() -> Vec<u8> {
    crate::entity_compact_codec::encode_delta(&delta()).unwrap()
}

fn publication_bytes(publication: &EntityPublication) -> Vec<u8> {
    encode(publication).unwrap()
}

fn append_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[test]
fn creation_and_delta_remove_only_the_global_coordinate_and_use_context() {
    let creation_publication = EntityPublication::Creation(creation());
    let creation_bytes = publication_bytes(&creation_publication);
    let mut expected_creation = legacy_creation_body();
    expected_creation[..8].copy_from_slice(b"CVAENC02");
    expected_creation.drain(57..65); // ID + optional UUID precede the global coordinate
    assert_eq!(creation_bytes, expected_creation);
    assert_eq!(
        decode(&creation_bytes, 701).unwrap(),
        Some(EntityPublication::Creation({
            let mut value = creation();
            value.record.global_version = 701;
            value
        }))
    );

    let delta_publication = EntityPublication::Delta(delta());
    let delta_bytes = publication_bytes(&delta_publication);
    let mut expected_delta = legacy_delta_body();
    expected_delta[..8].copy_from_slice(b"CVAEND02");
    expected_delta.drain(56..64); // global coordinate supplied by the envelope
    assert_eq!(delta_bytes, expected_delta);
    assert_eq!(
        decode(&delta_bytes, 702).unwrap(),
        Some(EntityPublication::Delta({
            let mut value = delta();
            value.global_version = 702;
            value
        }))
    );
}

#[test]
fn creation_and_delta_bytes_do_not_depend_on_global_context() {
    let mut first = record();
    first.global_version = 41;
    let mut second = first.clone();
    second.global_version = 9001;
    assert_eq!(
        encode(&EntityPublication::Creation(
            EntityCreation::new(first).unwrap()
        ))
        .unwrap(),
        encode(&EntityPublication::Creation(
            EntityCreation::new(second).unwrap()
        ))
        .unwrap()
    );

    let previous = record();
    let mut first_next = next_record(&previous);
    let mut second_next = first_next.clone();
    first_next.global_version = 42;
    second_next.global_version = 9001;
    assert_eq!(
        encode(&EntityPublication::Delta(
            EntityDelta::between(&previous, &first_next).unwrap()
        ))
        .unwrap(),
        encode(&EntityPublication::Delta(
            EntityDelta::between(&previous, &second_next).unwrap()
        ))
        .unwrap()
    );
}

#[test]
fn full_and_zero_mask_deltas_round_trip() {
    let previous = record();
    let mut changed = next_record(&previous);
    changed.global_id = Some(EntityGlobalId([0xA4; 16]));
    changed.aliases = vec!["new alias".into()];
    changed.kind = "service".into();
    changed.summary = "new summary".into();
    changed.updated_at_ns = 11;
    let full = EntityDelta::between(&previous, &changed).unwrap();
    assert_eq!(full.fields.mask(), ENTITY_DELTA_KNOWN_MASK);
    let full_publication = EntityPublication::Delta(full.clone());
    assert_eq!(
        decode(&publication_bytes(&full_publication), 42).unwrap(),
        Some({
            let mut value = full;
            value.global_version = 42;
            EntityPublication::Delta(value)
        })
    );

    let mut unchanged = next_record(&previous);
    unchanged.canonical_name = previous.canonical_name.clone();
    unchanged.aliases = previous.aliases.clone();
    unchanged.kind = previous.kind.clone();
    unchanged.summary = previous.summary.clone();
    unchanged.updated_at_ns = previous.updated_at_ns;
    let empty = EntityDelta::between(&previous, &unchanged).unwrap();
    assert_eq!(empty.fields.mask(), 0);
    let empty_publication = EntityPublication::Delta(empty.clone());
    assert_eq!(
        decode(&publication_bytes(&empty_publication), 42).unwrap(),
        Some({
            let mut value = empty;
            value.global_version = 42;
            EntityPublication::Delta(value)
        })
    );

    let mut unknown_mask = publication_bytes(&EntityPublication::Delta(delta()));
    let mutation_length = u32::from_le_bytes(unknown_mask[64..68].try_into().unwrap()) as usize;
    let mask_at = 68 + mutation_length;
    unknown_mask[mask_at..mask_at + 8].copy_from_slice(&(1_u64 << 5).to_le_bytes());
    assert!(decode(&unknown_mask, 42).is_err());
}

#[test]
fn authenticated_pruned_creation_and_delta_round_trip_and_check_body_checksum() {
    let proof = EntityDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let mut pruned = record();
    pruned.canonical_name = "[pruned]".into();
    pruned.aliases.clear();
    pruned.summary.clear();
    let pruned_creation = EntityCreation::new_pruned(pruned.clone(), proof.clone()).unwrap();
    let creation_bytes = publication_bytes(&EntityPublication::Creation(pruned_creation.clone()));
    assert_eq!(
        decode(&creation_bytes, pruned.global_version).unwrap(),
        Some(EntityPublication::Creation(pruned_creation))
    );
    let mut tampered_body = creation_bytes.clone();
    tampered_body[8] ^= 1; // Keep a structurally valid ID mutation; checksum must cover body.
    assert!(decode(&tampered_body, pruned.global_version).is_err());
    let mut tampered_creation = creation_bytes;
    let checksum_index = tampered_creation.len() - 1;
    tampered_creation[checksum_index] ^= 1;
    assert!(decode(&tampered_creation, pruned.global_version).is_err());

    let previous = record();
    pruned.revision = 2;
    pruned.global_version = 42;
    pruned.entity_version = 5;
    pruned.mutation_id = "mutation-pruned".into();
    let pruned_delta = EntityDelta::between_pruned(&previous, &pruned, proof).unwrap();
    let delta_bytes = publication_bytes(&EntityPublication::Delta(pruned_delta.clone()));
    assert_eq!(
        decode(&delta_bytes, pruned.global_version).unwrap(),
        Some(EntityPublication::Delta(pruned_delta))
    );
    let mut tampered_delta = delta_bytes;
    let checksum_index = tampered_delta.len() - 1;
    tampered_delta[checksum_index] ^= 1;
    assert!(decode(&tampered_delta, pruned.global_version).is_err());
}

#[test]
fn terminal_publications_round_trip_with_global_context() {
    let direct = EntityPublication::DirectTombstone {
        id: EntityId([3; 32]),
        revision: 4,
        entity_version: 8,
    };
    let direct_bytes = publication_bytes(&direct);
    let mut expected_direct = b"CVAETD02".to_vec();
    expected_direct.extend_from_slice(&[3; 32]);
    append_u64(&mut expected_direct, 4);
    append_u64(&mut expected_direct, 8);
    assert_eq!(direct_bytes, expected_direct);
    assert_eq!(decode(&direct_bytes, 99).unwrap(), Some(direct));

    let merge = EntityPublication::MergeTombstone {
        id: EntityId([4; 32]),
        revision: 6,
        entity_version: 10,
        replacement_id: EntityId([5; 32]),
    };
    let merge_bytes = publication_bytes(&merge);
    let mut expected_merge = b"CVAETM02".to_vec();
    expected_merge.extend_from_slice(&[4; 32]);
    append_u64(&mut expected_merge, 6);
    append_u64(&mut expected_merge, 10);
    expected_merge.extend_from_slice(&[5; 32]);
    assert_eq!(merge_bytes, expected_merge);
    assert_eq!(decode(&merge_bytes, 100).unwrap(), Some(merge));
}

#[test]
fn recognized_publications_reject_unknown_truncated_trailing_and_zero_coordinates() {
    let publications = [
        EntityPublication::Creation(creation()),
        EntityPublication::Delta(delta()),
        EntityPublication::DirectTombstone {
            id: EntityId([3; 32]),
            revision: 4,
            entity_version: 8,
        },
        EntityPublication::MergeTombstone {
            id: EntityId([4; 32]),
            revision: 6,
            entity_version: 10,
            replacement_id: EntityId([5; 32]),
        },
    ];
    for publication in publications {
        let bytes = publication_bytes(&publication);
        for end in 6..bytes.len() {
            assert!(decode(&bytes[..end], 1).is_err(), "prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0xEE);
        assert!(decode(&trailing, 1).is_err());
        assert!(decode(&bytes, 0).is_err(), "zero global context");
        let mut unknown = bytes;
        unknown[7] = b'9';
        assert!(decode(&unknown, 1).is_err());
    }

    for (publication, offset) in [
        (
            EntityPublication::DirectTombstone {
                id: EntityId([3; 32]),
                revision: 4,
                entity_version: 8,
            },
            40,
        ),
        (
            EntityPublication::DirectTombstone {
                id: EntityId([3; 32]),
                revision: 4,
                entity_version: 8,
            },
            48,
        ),
        (
            EntityPublication::MergeTombstone {
                id: EntityId([4; 32]),
                revision: 6,
                entity_version: 10,
                replacement_id: EntityId([5; 32]),
            },
            40,
        ),
        (
            EntityPublication::MergeTombstone {
                id: EntityId([4; 32]),
                revision: 6,
                entity_version: 10,
                replacement_id: EntityId([5; 32]),
            },
            48,
        ),
    ] {
        let mut bytes = publication_bytes(&publication);
        bytes[offset..offset + 8].fill(0);
        assert!(decode(&bytes, 1).is_err());
    }

    let mut creation_bytes = publication_bytes(&EntityPublication::Creation(creation()));
    creation_bytes[57..65].fill(0); // local Entity sequence
    assert!(decode(&creation_bytes, 1).is_err());
    for offset in [40, 48, 56] {
        // prior revision, revision, local Entity sequence
        let mut delta_bytes = publication_bytes(&EntityPublication::Delta(delta()));
        delta_bytes[offset..offset + 8].fill(0);
        assert!(
            decode(&delta_bytes, 1).is_err(),
            "delta coordinate {offset}"
        );
    }
}

#[test]
fn legacy_compact_frames_are_rejected_and_unrelated_owner_payloads_are_ignored() {
    for mut bytes in [legacy_creation_body(), legacy_delta_body()] {
        assert!(decode(&bytes, 1).is_err());
        bytes.push(0);
        assert!(decode(&bytes, 1).is_err());
    }
    assert_eq!(decode(b"CVAMEMC2 unrelated Memory body", 1).unwrap(), None);
    assert_eq!(
        decode(b"CVAENTR1 unrelated Entity record", 1).unwrap(),
        None
    );
    assert_eq!(decode(b"ordinary container payload", 1).unwrap(), None);
}

#[test]
fn encoder_rejects_zero_local_coordinates() {
    let mut invalid = delta();
    invalid.entity_version = 0;
    assert!(encode(&EntityPublication::Delta(invalid)).is_err());
}
