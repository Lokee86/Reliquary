use super::compact_description_proof;
fn encode_creation(c: &RelationshipCreation) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Creation(
            c.clone(),
        ),
    )
}
fn encode_delta(d: &RelationshipDelta) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Delta(d.clone()),
    )
}
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::{EntityId, EntityRef, RelationshipId, RelationshipMemoryRef, RelationshipParticipant};

fn record(revision: u64, mutation_id: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x31; 32]),
        revision,
        kind: "observes".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "external".into(),
                entity_id: EntityId([0x32; 32]),
            },
            role: None,
        }],
        evidence: vec![RelationshipMemoryRef {
            owner_id: "external".into(),
            memory_id: [0x33; 32],
        }],
        summary: String::new(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 11,
        global_version: revision,
        relationship_version: revision,
    }
}

fn proof() -> RelationshipDescriptionProof {
    RelationshipDescriptionProof {
        original_carrier_digest: [0xA1; 32],
        manifest_digest: [0xB2; 32],
    }
}

#[test]
fn extracts_proof_identity_from_creation_and_delta_carriers() {
    let creation = RelationshipCreation::new_pruned(record(1, "create"), proof()).unwrap();
    let creation_bytes = encode_creation(&creation).unwrap();
    assert_eq!(
        compact_description_proof(&creation_bytes).unwrap(),
        Some((creation.record.id, 1, proof()))
    );

    let previous = record(1, "create");
    let mut next = record(2, "update");
    next.updated_at_ns = 12;
    let delta = RelationshipDelta::between_pruned(&previous, &next, proof()).unwrap();
    let delta_bytes = encode_delta(&delta).unwrap();
    assert_eq!(
        compact_description_proof(&delta_bytes).unwrap(),
        Some((delta.id, delta.revision, proof()))
    );
}

#[test]
fn authored_placeholder_and_unrelated_payload_have_no_proof() {
    let creation = RelationshipCreation::new(record(1, "authored-placeholder")).unwrap();
    assert_eq!(
        compact_description_proof(&encode_creation(&creation).unwrap()).unwrap(),
        None
    );
    assert_eq!(
        compact_description_proof(b"not a Relationship carrier").unwrap(),
        None
    );
}

#[test]
fn unknown_versions_and_checksum_damage_fail_closed() {
    assert!(compact_description_proof(b"CVARNC99unknown version").is_err());

    let creation = RelationshipCreation::new_pruned(record(1, "create"), proof()).unwrap();
    let mut bytes = encode_creation(&creation).unwrap();
    let checksum_at = bytes.len() - 32;
    bytes[checksum_at] ^= 1;
    assert!(compact_description_proof(&bytes).is_err());
}
