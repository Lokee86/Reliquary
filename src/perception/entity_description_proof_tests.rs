use super::compact_description_proof;
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDescriptionProof};
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

fn record(revision: u64, mutation_id: &str) -> EntityRecord {
    EntityRecord {
        id: EntityId([0x31; 32]),
        global_id: Some(EntityGlobalId([0x32; 16])),
        revision,
        canonical_name: "[pruned]".into(),
        aliases: Vec::new(),
        kind: "project".into(),
        summary: String::new(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 11,
        global_version: revision,
        entity_version: revision,
    }
}

fn proof() -> EntityDescriptionProof {
    EntityDescriptionProof {
        original_carrier_digest: [0xA1; 32],
        manifest_digest: [0xB2; 32],
    }
}

#[test]
fn extracts_proof_identity_from_creation_and_delta_carriers() {
    let creation = EntityCreation::new_pruned(record(1, "create"), proof()).unwrap();
    let creation_bytes = encode_creation(&creation).unwrap();
    assert_eq!(
        compact_description_proof(&creation_bytes).unwrap(),
        Some((creation.record.id, 1, proof()))
    );

    let previous = record(1, "create");
    let mut next = record(2, "update");
    next.updated_at_ns = 12;
    let delta = EntityDelta::between_pruned(&previous, &next, proof()).unwrap();
    let delta_bytes = encode_delta(&delta).unwrap();
    assert_eq!(
        compact_description_proof(&delta_bytes).unwrap(),
        Some((delta.id, delta.revision, proof()))
    );
}

#[test]
fn authored_placeholder_and_unrelated_payload_have_no_proof() {
    let creation = EntityCreation::new(record(1, "authored-placeholder")).unwrap();
    assert_eq!(
        compact_description_proof(&encode_creation(&creation).unwrap()).unwrap(),
        None
    );
    assert_eq!(
        compact_description_proof(b"not an Entity carrier").unwrap(),
        None
    );
}

#[test]
fn unknown_versions_and_checksum_damage_fail_closed() {
    assert!(compact_description_proof(b"CVAENC99unknown version").is_err());

    let creation = EntityCreation::new_pruned(record(1, "create"), proof()).unwrap();
    let mut bytes = encode_creation(&creation).unwrap();
    let checksum_at = bytes.len() - 32;
    bytes[checksum_at] ^= 1;
    assert!(compact_description_proof(&bytes).is_err());
}
