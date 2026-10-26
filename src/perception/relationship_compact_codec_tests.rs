use super::*;
use crate::relationship_compact_contract::{
    RelationshipCollectionChange, RelationshipCreation, RelationshipDelta, RelationshipDeltaFields,
    RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::{EntityId, EntityRef, MemoryId, MemoryRef, RelationshipId, RelationshipParticipant};
use reliquary_container::codec::FieldChange;

fn participant(owner: &str, entity: u8, role: Option<&str>) -> RelationshipParticipant {
    RelationshipParticipant {
        entity: EntityRef {
            owner_id: owner.into(),
            entity_id: EntityId([entity; 32]),
        },
        role: role.map(str::to_owned),
    }
}

fn memory(owner: &str, id: u8) -> MemoryRef {
    MemoryRef {
        owner_id: owner.into(),
        memory_id: MemoryId([id; 32]),
    }
}

fn record() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([1; 32]),
        revision: 1,
        kind: "k".into(),
        participants: vec![participant("o", 2, None)],
        evidence: Vec::new(),
        summary: "s".into(),
        mutation_id: "m".into(),
        created_at_ns: 3,
        updated_at_ns: 4,
        global_version: 5,
        relationship_version: 6,
    }
}

fn delta_to(mut next: RelationshipRecord) -> RelationshipDelta {
    let previous = record();
    next.id = previous.id;
    next.revision = 2;
    next.global_version = 7;
    next.relationship_version = 8;
    next.mutation_id = "n".into();
    RelationshipDelta::between(&previous, &next).unwrap()
}

fn successor() -> RelationshipDelta {
    let mut next = record();
    next.summary.clear();
    next.updated_at_ns = 5;
    delta_to(next)
}

fn creation_golden() -> Vec<u8> {
    let mut bytes = b"CVARNC01".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&5_u64.to_le_bytes());
    bytes.extend_from_slice(&6_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(b'm');
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(b'k');
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(b'o');
    bytes.extend_from_slice(&[2; 32]);
    bytes.push(0); // no participant role
    bytes.extend_from_slice(&0_u16.to_le_bytes()); // no evidence
    bytes.extend_from_slice(&1_u32.to_le_bytes()); // exact summary
    bytes.push(b's');
    bytes.extend_from_slice(&3_i64.to_le_bytes());
    bytes.extend_from_slice(&4_i64.to_le_bytes());
    bytes.push(0); // no prune proof, therefore no checksum
    bytes
}

fn delta_golden() -> Vec<u8> {
    let mut bytes = b"CVARND01".to_vec();
    bytes.extend_from_slice(&[1; 32]);
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    bytes.extend_from_slice(&7_u64.to_le_bytes());
    bytes.extend_from_slice(&8_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(b'n');
    bytes.extend_from_slice(&24_u64.to_le_bytes()); // summary clear + updated time
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // empty summary
    bytes.extend_from_slice(&5_i64.to_le_bytes());
    bytes.push(0); // no prune proof, therefore no checksum
    bytes
}

#[test]
fn ordinary_creation_and_delta_match_golden_frames_without_checksum() {
    let creation = encode_creation(&RelationshipCreation::new(record()).unwrap()).unwrap();
    assert_eq!(creation, creation_golden());
    assert_eq!(decode_creation(&creation).unwrap().record, record());
    assert!(decode_creation(&[creation.clone(), [0_u8; 32].to_vec()].concat()).is_err());

    let delta = encode_delta(&successor()).unwrap();
    assert_eq!(delta, delta_golden());
    assert_eq!(decode_delta(&delta).unwrap(), successor());
    assert!(decode_delta(&[delta, [0_u8; 32].to_vec()].concat()).is_err());
}

#[test]
fn all_thirty_two_masks_round_trip_and_empty_mask_is_legal() {
    for mask in 0_u64..32 {
        let mut next = record();
        if mask & 1 != 0 {
            next.kind = "k2".into();
        }
        if mask & 2 != 0 {
            next.participants = vec![participant("p", 3, Some("role"))];
        }
        if mask & 4 != 0 {
            next.evidence = vec![memory("e", 4)];
        }
        if mask & 8 != 0 {
            next.summary = String::new();
        }
        if mask & 16 != 0 {
            next.updated_at_ns = 5;
        }
        let delta = delta_to(next);
        assert_eq!(delta.fields.mask(), mask);
        let bytes = encode_delta(&delta).unwrap();
        let decoded = decode_delta(&bytes).unwrap();
        assert_eq!(decoded.fields.mask(), mask);
        assert_eq!(decoded, delta);
    }
}

#[test]
fn participant_role_change_is_an_exact_remove_add_tuple_operation() {
    let mut next = record();
    next.participants = vec![participant("o", 2, Some("speaker"))];
    let delta = delta_to(next);
    assert_eq!(
        delta.fields.participants.removed,
        vec![participant("o", 2, None)]
    );
    assert_eq!(
        delta.fields.participants.added,
        vec![participant("o", 2, Some("speaker"))]
    );
    let decoded = decode_delta(&encode_delta(&delta).unwrap()).unwrap();
    assert_eq!(decoded.fields.participants, delta.fields.participants);
}

#[test]
fn proof_creation_and_delta_check_both_proof_digests_and_checksum() {
    let proof = RelationshipDescriptionProof {
        original_carrier_digest: [0xA1; 32],
        manifest_digest: [0xB2; 32],
    };
    let mut redacted = record();
    redacted.summary.clear();
    let creation = RelationshipCreation::new_pruned(redacted.clone(), proof.clone()).unwrap();
    let creation_bytes = encode_creation(&creation).unwrap();
    assert_eq!(decode_creation(&creation_bytes).unwrap(), creation);
    for offset in [
        creation_bytes.len() - 96,
        creation_bytes.len() - 64,
        creation_bytes.len() - 1,
    ] {
        let mut corrupted = creation_bytes.clone();
        corrupted[offset] ^= 1;
        assert!(decode_creation(&corrupted).is_err());
    }

    let mut next = redacted.clone();
    next.revision = 2;
    next.global_version = 7;
    next.relationship_version = 8;
    next.mutation_id = "n".into();
    next.updated_at_ns = 5;
    let delta = RelationshipDelta::between_pruned(&redacted, &next, proof).unwrap();
    let delta_bytes = encode_delta(&delta).unwrap();
    assert_eq!(decode_delta(&delta_bytes).unwrap(), delta);
    for offset in [
        delta_bytes.len() - 96,
        delta_bytes.len() - 64,
        delta_bytes.len() - 1,
    ] {
        let mut corrupted = delta_bytes.clone();
        corrupted[offset] ^= 1;
        assert!(decode_delta(&corrupted).is_err());
    }
}

#[test]
fn proof_bearing_delta_rejects_nonempty_summary_set_but_accepts_unchanged_summary() {
    let mut delta = successor();
    delta.description_proof = Some(RelationshipDescriptionProof {
        original_carrier_digest: [1; 32],
        manifest_digest: [2; 32],
    });
    delta.fields.summary = FieldChange::Set("still authored".into());
    assert!(encode_delta(&delta).is_err());

    let mut redacted = record();
    redacted.summary.clear();
    let mut next = redacted.clone();
    next.revision = 2;
    next.global_version = 7;
    next.relationship_version = 8;
    next.mutation_id = "n".into();
    next.updated_at_ns = 5;
    let proof_delta = RelationshipDelta::between_pruned(
        &redacted,
        &next,
        RelationshipDescriptionProof {
            original_carrier_digest: [3; 32],
            manifest_digest: [4; 32],
        },
    )
    .unwrap();
    assert!(matches!(proof_delta.fields.summary, FieldChange::Unchanged));
    assert!(decode_delta(&encode_delta(&proof_delta).unwrap()).is_ok());
}

#[test]
fn every_proper_prefix_of_creation_and_delta_frames_is_rejected() {
    let creation = encode_creation(&RelationshipCreation::new(record()).unwrap()).unwrap();
    for length in 0..creation.len() {
        assert!(
            decode_creation(&creation[..length]).is_err(),
            "creation prefix {length}"
        );
    }
    let delta = encode_delta(&successor()).unwrap();
    for length in 0..delta.len() {
        assert!(
            decode_delta(&delta[..length]).is_err(),
            "delta prefix {length}"
        );
    }
}

#[test]
fn unknown_versions_invalid_utf8_lengths_counts_tags_masks_and_envelopes_fail_closed() {
    let mut unknown_creation = b"CVARNC99".to_vec();
    unknown_creation.extend_from_slice(&[0; 32]);
    assert!(decode_creation(&unknown_creation).is_err());
    assert!(is_compact_record(&unknown_creation).is_err());
    let mut unknown_delta = b"CVARND99".to_vec();
    unknown_delta.extend_from_slice(&[0; 32]);
    assert!(decode_delta(&unknown_delta).is_err());
    assert!(is_compact_record(&unknown_delta).is_err());

    let creation = encode_creation(&RelationshipCreation::new(record()).unwrap()).unwrap();
    let mut invalid_utf8 = creation.clone();
    invalid_utf8[60] = 0xFF; // first byte of mutation_id
    assert!(decode_creation(&invalid_utf8).is_err());

    let mut oversized_string = creation.clone();
    oversized_string[56..60].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_creation(&oversized_string).is_err());

    let mut too_many_participants = creation.clone();
    too_many_participants[66..68]
        .copy_from_slice(&((crate::MAX_RELATIONSHIP_PARTICIPANTS + 1) as u16).to_le_bytes());
    assert!(decode_creation(&too_many_participants).is_err());

    let mut too_many_evidence = creation.clone();
    too_many_evidence[106..108]
        .copy_from_slice(&((crate::MAX_RELATIONSHIP_EVIDENCE + 1) as u16).to_le_bytes());
    assert!(decode_creation(&too_many_evidence).is_err());

    let mut invalid_role_tag = creation.clone();
    invalid_role_tag[105] = 2;
    assert!(decode_creation(&invalid_role_tag).is_err());

    let mut invalid_proof_tag = creation;
    *invalid_proof_tag.last_mut().unwrap() = 2;
    assert!(decode_creation(&invalid_proof_tag).is_err());

    let mut delta = encode_delta(&successor()).unwrap();
    let mask_offset = 77;
    delta[mask_offset..mask_offset + 8].copy_from_slice(&(1_u64 << 5).to_le_bytes());
    assert!(decode_delta(&delta).is_err());

    let good = encode_delta(&successor()).unwrap();
    for (offset, value) in [
        (40, 0_u64), // previous revision zero
        (48, 3_u64), // not the exact successor
        (56, 0_u64), // global publication coordinate zero
        (64, 0_u64), // local publication coordinate zero
    ] {
        let mut invalid = good.clone();
        invalid[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(decode_delta(&invalid).is_err());
    }
}

fn write_test_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn write_test_participant(out: &mut Vec<u8>, value: &RelationshipParticipant) {
    write_test_string(out, &value.entity.owner_id);
    out.extend_from_slice(&value.entity.entity_id.0);
    match &value.role {
        None => out.push(0),
        Some(role) => {
            out.push(1);
            write_test_string(out, role);
        }
    }
}

fn write_test_memory(out: &mut Vec<u8>, value: &MemoryRef) {
    write_test_string(out, &value.owner_id);
    out.extend_from_slice(&value.memory_id.0);
}

fn malformed_collection_delta(
    participant_mask: bool,
    removed_participants: &[RelationshipParticipant],
    added_participants: &[RelationshipParticipant],
    removed_evidence: &[MemoryRef],
    added_evidence: &[MemoryRef],
) -> Vec<u8> {
    let mut bytes = b"CVARND01".to_vec();
    bytes.extend_from_slice(&[9; 32]);
    for value in [1_u64, 2, 3, 4] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    write_test_string(&mut bytes, "mutation");
    let mask = if participant_mask { 1 << 1 } else { 1 << 2 };
    bytes.extend_from_slice(&(mask as u64).to_le_bytes());
    if participant_mask {
        bytes.extend_from_slice(&(removed_participants.len() as u16).to_le_bytes());
        for value in removed_participants {
            write_test_participant(&mut bytes, value);
        }
        bytes.extend_from_slice(&(added_participants.len() as u16).to_le_bytes());
        for value in added_participants {
            write_test_participant(&mut bytes, value);
        }
    } else {
        bytes.extend_from_slice(&(removed_evidence.len() as u16).to_le_bytes());
        for value in removed_evidence {
            write_test_memory(&mut bytes, value);
        }
        bytes.extend_from_slice(&(added_evidence.len() as u16).to_le_bytes());
        for value in added_evidence {
            write_test_memory(&mut bytes, value);
        }
    }
    bytes.push(0); // no proof
    bytes
}

#[test]
fn malformed_collection_operations_reject_unsorted_duplicates_overlap_and_empty_bits() {
    let low_participant = participant("a", 1, None);
    let high_participant = participant("z", 2, None);
    let low_memory = memory("a", 1);
    let high_memory = memory("z", 2);

    let malformed = [
        malformed_collection_delta(
            true,
            &[high_participant.clone(), low_participant.clone()],
            &[],
            &[],
            &[],
        ),
        malformed_collection_delta(
            true,
            &[low_participant.clone(), low_participant.clone()],
            &[],
            &[],
            &[],
        ),
        malformed_collection_delta(
            true,
            &[low_participant.clone()],
            &[low_participant.clone()],
            &[],
            &[],
        ),
        malformed_collection_delta(true, &[], &[], &[], &[]),
        malformed_collection_delta(
            false,
            &[],
            &[],
            &[high_memory.clone(), low_memory.clone()],
            &[],
        ),
        malformed_collection_delta(
            false,
            &[],
            &[],
            &[low_memory.clone(), low_memory.clone()],
            &[],
        ),
        malformed_collection_delta(
            false,
            &[],
            &[],
            &[low_memory.clone()],
            &[low_memory.clone()],
        ),
        malformed_collection_delta(false, &[], &[], &[], &[]),
    ];
    for bytes in malformed {
        assert!(decode_delta(&bytes).is_err());
    }

    let mut too_many_ops = b"CVARND01".to_vec();
    too_many_ops.extend_from_slice(&[9; 32]);
    for value in [1_u64, 2, 3, 4] {
        too_many_ops.extend_from_slice(&value.to_le_bytes());
    }
    write_test_string(&mut too_many_ops, "mutation");
    too_many_ops.extend_from_slice(&(1_u64 << 1).to_le_bytes());
    too_many_ops
        .extend_from_slice(&((crate::MAX_RELATIONSHIP_PARTICIPANTS + 1) as u16).to_le_bytes());
    assert!(decode_delta(&too_many_ops).is_err());
}

#[test]
fn malformed_summary_and_delta_tail_are_rejected() {
    let mut delta = successor();
    delta.fields.kind = FieldChange::Set(String::new());
    assert!(encode_delta(&delta).is_err());

    let mut trailing = encode_delta(&successor()).unwrap();
    trailing.extend_from_slice(b"junk");
    assert!(decode_delta(&trailing).is_err());
}

#[allow(dead_code)]
fn _contract_field_shape_is_exhaustive(fields: RelationshipDeltaFields) {
    let RelationshipDeltaFields {
        kind: _,
        participants:
            RelationshipCollectionChange {
                removed: _,
                added: _,
            },
        evidence:
            RelationshipCollectionChange {
                removed: _,
                added: _,
            },
        summary: _,
        updated_at_ns: _,
    } = fields;
}
