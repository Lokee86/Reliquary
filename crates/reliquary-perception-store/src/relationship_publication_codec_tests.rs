use crate::relationship_compact_codec::publication::{RelationshipPublication, decode, encode};
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{EntityId, EntityRef, RelationshipId, RelationshipMemoryRef};

fn participant() -> RelationshipParticipant {
    RelationshipParticipant {
        entity: EntityRef {
            owner_id: "owner-a".into(),
            entity_id: EntityId([2; 32]),
        },
        role: Some("source".into()),
    }
}

fn record() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([1; 32]),
        revision: 1,
        kind: "supports".into(),
        participants: vec![participant()],
        evidence: vec![RelationshipMemoryRef {
            owner_id: "owner-b".into(),
            memory_id: [3; 32],
        }],
        summary: "summary".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: -7,
        updated_at_ns: 9,
        global_version: 41,
        relationship_version: 3,
    }
}

fn next_record(previous: &RelationshipRecord) -> RelationshipRecord {
    let mut next = previous.clone();
    next.revision = previous.revision + 1;
    next.global_version = 42;
    next.relationship_version = 5;
    next.mutation_id = "mutation-2".into();
    next.kind = "depends-on".into();
    next.updated_at_ns = 11;
    next
}

fn proof() -> RelationshipDescriptionProof {
    RelationshipDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    }
}

fn append_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn creation_publication() -> RelationshipPublication {
    RelationshipPublication::Creation(RelationshipCreation::new(record()).unwrap())
}

fn delta_publication() -> RelationshipPublication {
    let previous = record();
    RelationshipPublication::Delta(
        RelationshipDelta::between(&previous, &next_record(&previous)).unwrap(),
    )
}

#[test]
fn creation_and_delta_omit_only_global_coordinate() {
    let creation = match creation_publication() {
        RelationshipPublication::Creation(value) => value,
        _ => unreachable!(),
    };
    let mut expected_creation =
        crate::relationship_compact_codec::encode_creation(&creation).unwrap();
    expected_creation[..8].copy_from_slice(b"CVARNC02");
    expected_creation.drain(40..48);
    let creation_bytes = encode(&RelationshipPublication::Creation(creation.clone())).unwrap();
    assert_eq!(creation_bytes, expected_creation);
    assert_eq!(
        creation_bytes.len() + 8,
        crate::relationship_compact_codec::encode_creation(&creation)
            .unwrap()
            .len()
    );
    let mut contextual_creation = creation.clone();
    contextual_creation.record.global_version = 701;
    assert_eq!(
        creation_bytes,
        encode(&RelationshipPublication::Creation(
            contextual_creation.clone()
        ))
        .unwrap()
    );
    assert_eq!(
        decode(&creation_bytes, 701).unwrap(),
        Some(RelationshipPublication::Creation(contextual_creation))
    );

    let delta = match delta_publication() {
        RelationshipPublication::Delta(value) => value,
        _ => unreachable!(),
    };
    let mut expected_delta = crate::relationship_compact_codec::encode_delta(&delta).unwrap();
    expected_delta[..8].copy_from_slice(b"CVARND02");
    expected_delta.drain(56..64);
    let delta_bytes = encode(&RelationshipPublication::Delta(delta.clone())).unwrap();
    assert_eq!(delta_bytes, expected_delta);
    assert_eq!(
        delta_bytes.len() + 8,
        crate::relationship_compact_codec::encode_delta(&delta)
            .unwrap()
            .len()
    );
    let mut contextual_delta = delta.clone();
    contextual_delta.global_version = 702;
    assert_eq!(
        delta_bytes,
        encode(&RelationshipPublication::Delta(contextual_delta.clone())).unwrap()
    );
    assert_eq!(
        decode(&delta_bytes, 702).unwrap(),
        Some(RelationshipPublication::Delta(contextual_delta))
    );
}

#[test]
fn proof_bearing_creation_and_delta_round_trip_and_bind_body_checksum() {
    let mut pruned = record();
    pruned.summary.clear();
    let creation = RelationshipCreation::new_pruned(pruned.clone(), proof()).unwrap();
    let bytes = encode(&RelationshipPublication::Creation(creation.clone())).unwrap();
    assert_eq!(
        decode(&bytes, pruned.global_version).unwrap(),
        Some(RelationshipPublication::Creation(creation))
    );
    let mut changed_body = bytes.clone();
    changed_body[8] ^= 1; // ID remains structurally valid; checksum must cover body bytes.
    assert!(decode(&changed_body, pruned.global_version).is_err());
    assert!(decode(&bytes, pruned.global_version + 100).is_ok());
    let mut changed_digest = bytes.clone();
    let proof_start = changed_digest.len() - 97;
    changed_digest[proof_start + 1] ^= 1;
    assert!(decode(&changed_digest, pruned.global_version).is_err());
    let mut changed_manifest = bytes.clone();
    changed_manifest[proof_start + 33] ^= 1;
    assert!(decode(&changed_manifest, pruned.global_version).is_err());
    let mut changed_checksum = bytes.clone();
    *changed_checksum.last_mut().unwrap() ^= 1;
    assert!(decode(&changed_checksum, pruned.global_version).is_err());

    let previous = record();
    pruned.revision = 2;
    pruned.global_version = 42;
    pruned.relationship_version = 5;
    pruned.mutation_id = "mutation-pruned".into();
    pruned.updated_at_ns = 11;
    let delta = RelationshipDelta::between_pruned(&previous, &pruned, proof()).unwrap();
    let delta_bytes = encode(&RelationshipPublication::Delta(delta.clone())).unwrap();
    assert_eq!(
        decode(&delta_bytes, 42).unwrap(),
        Some(RelationshipPublication::Delta(delta))
    );
    let mut corrupt_delta = delta_bytes;
    *corrupt_delta.last_mut().unwrap() ^= 1;
    assert!(decode(&corrupt_delta, 42).is_err());
}

#[test]
fn empty_delta_round_trips_and_unknown_mask_bits_fail_closed() {
    let previous = record();
    let mut next = previous.clone();
    next.revision = 2;
    next.global_version = 42;
    next.relationship_version = 5;
    next.mutation_id = "empty-change".into();
    let empty = RelationshipDelta::between(&previous, &next).unwrap();
    assert_eq!(empty.fields.mask(), 0);
    let publication = RelationshipPublication::Delta(empty);
    let bytes = encode(&publication).unwrap();
    assert_eq!(decode(&bytes, 42).unwrap(), Some(publication));

    let mutation_length = u32::from_le_bytes(bytes[64..68].try_into().unwrap()) as usize;
    let mask_at = 68 + mutation_length;
    let mut unknown_mask = bytes;
    unknown_mask[mask_at..mask_at + 8].copy_from_slice(&(1_u64 << 5).to_le_bytes());
    assert!(decode(&unknown_mask, 42).is_err());
}

#[test]
fn direct_tombstone_keeps_local_version_and_uses_envelope_global() {
    let tombstone = RelationshipPublication::Tombstone {
        id: RelationshipId([8; 32]),
        revision: 4,
        relationship_version: 9,
    };
    let mut expected = b"CVARTD02".to_vec();
    expected.extend_from_slice(&[8; 32]);
    append_u64(&mut expected, 4);
    append_u64(&mut expected, 9);
    let bytes = encode(&tombstone).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(bytes.len(), 56);
    assert_eq!(decode(&bytes, 99).unwrap(), Some(tombstone));
    assert!(decode(&bytes, 0).is_err());
}

#[test]
fn recognized_publications_reject_truncation_trailing_unknown_version_and_zero_local() {
    let mut tombstone = b"CVARTD02".to_vec();
    tombstone.extend_from_slice(&[8; 32]);
    append_u64(&mut tombstone, 4);
    append_u64(&mut tombstone, 9);
    let publications = [
        creation_publication(),
        delta_publication(),
        RelationshipPublication::Tombstone {
            id: RelationshipId([8; 32]),
            revision: 4,
            relationship_version: 9,
        },
    ];
    for publication in publications {
        let bytes = encode(&publication).unwrap();
        for end in 8..bytes.len() {
            assert!(decode(&bytes[..end], 1).is_err(), "prefix {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0xEE);
        assert!(decode(&trailing, 1).is_err());
        let mut unsupported = bytes.clone();
        unsupported[7] = b'9';
        assert!(decode(&unsupported, 1).is_err());
    }
    for prefix_len in [6, 7] {
        let prefix = b"CVARNC02"[..prefix_len].to_vec();
        assert!(
            decode(&prefix, 1).is_err(),
            "recognized family prefix {prefix_len}"
        );
        let prefix = b"CVARND02"[..prefix_len].to_vec();
        assert!(
            decode(&prefix, 1).is_err(),
            "recognized family prefix {prefix_len}"
        );
        let prefix = b"CVARTD02"[..prefix_len].to_vec();
        assert!(
            decode(&prefix, 1).is_err(),
            "recognized family prefix {prefix_len}"
        );
    }
    assert_eq!(decode(b"unrelated", 1).unwrap(), None);
    assert!(decode(&tombstone, 0).is_err());

    let mut zero_creation = encode(&creation_publication()).unwrap();
    zero_creation[40..48].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&zero_creation, 1).is_err());
    let mut zero_delta = encode(&delta_publication()).unwrap();
    zero_delta[56..64].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&zero_delta, 1).is_err());
    let mut zero_tombstone = tombstone;
    zero_tombstone[48..56].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&zero_tombstone, 1).is_err());

    let mut invalid_terminal_revision = zero_tombstone.clone();
    invalid_terminal_revision[40..48].copy_from_slice(&1_u64.to_le_bytes());
    assert!(decode(&invalid_terminal_revision, 1).is_err());
    let mut invalid_delta_successor = encode(&delta_publication()).unwrap();
    invalid_delta_successor[48..56].copy_from_slice(&3_u64.to_le_bytes());
    assert!(decode(&invalid_delta_successor, 1).is_err());
}
