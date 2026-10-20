//! Scratch checks for legacy-wire-compatible retained Relationship structure hashes.
use crate::relationship_codec::encode_record;
use crate::relationship_model::RelationshipRecord;
use crate::relationship_retained_structure::encode_record_structure;
use crate::{EntityId, EntityRef, MemoryId, MemoryRef, RelationshipId, RelationshipParticipant};
use sha2::{Digest, Sha256};

fn sample() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x31; 32]),
        revision: 7,
        kind: "  associé non-ASCII  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  équipe-a  ".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("  rôle Ω  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "遠端-entity".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![
            MemoryRef {
                owner_id: "  mémoire-a  ".into(),
                memory_id: MemoryId([0x33; 32]),
            },
            MemoryRef {
                owner_id: "遠端-memory".into(),
                memory_id: MemoryId([0x44; 32]),
            },
        ],
        summary: "PRIVATE DESCRIPTION THAT MUST NOT LEAK".into(),
        mutation_id: "  mutation Δ  ".into(),
        created_at_ns: 1_200,
        updated_at_ns: 1_234,
        global_version: 80,
        relationship_version: 12,
    }
}

fn bytes(record: &RelationshipRecord) -> Vec<u8> {
    encode_record_structure(record).unwrap()
}

#[test]
fn retained_structure_is_exact_legacy_carrier_hash_input_with_empty_summary() {
    let original = sample();
    let mut legacy_equivalent = original.clone();
    legacy_equivalent.summary.clear();

    let structure = bytes(&original);
    let legacy = encode_record(&legacy_equivalent).unwrap();
    assert_eq!(structure, legacy);
    assert_eq!(Sha256::digest(&structure), Sha256::digest(&legacy));
    assert!(structure.starts_with(b"CVARELR1"));
    assert!(
        !structure
            .windows(b"PRIVATE DESCRIPTION THAT MUST NOT LEAK".len())
            .any(|window| window == b"PRIVATE DESCRIPTION THAT MUST NOT LEAK"),
        "pruned summary bytes cannot appear in the retained hash input"
    );
    assert_eq!(
        String::from_utf8_lossy(&structure)
            .matches("  équipe-a  ")
            .count(),
        1
    );
    assert!(String::from_utf8_lossy(&structure).contains("  rôle Ω  "));
    assert!(String::from_utf8_lossy(&structure).contains("  mutation Δ  "));

    let mut relocated = original.clone();
    relocated.global_version = 800;
    relocated.relationship_version = 120;
    assert_eq!(
        bytes(&relocated),
        structure,
        "publication coordinates stay in the outer structure"
    );
}

#[test]
fn retained_structure_hash_input_changes_for_every_preserved_semantic_dimension() {
    let original = sample();
    let expected = bytes(&original);

    let mut changed = original.clone();
    changed.kind = "  supports relation  ".into();
    assert_ne!(bytes(&changed), expected, "kind is retained");

    let mut changed = original.clone();
    changed.participants[0].entity.owner_id = "  changed entity owner  ".into();
    assert_ne!(
        bytes(&changed),
        expected,
        "participant entity owner is retained"
    );

    let mut changed = original.clone();
    changed.participants[0].entity.entity_id = EntityId([0x66; 32]);
    assert_ne!(
        bytes(&changed),
        expected,
        "participant entity identity is retained"
    );

    let mut changed = original.clone();
    changed.participants.swap(0, 1);
    assert_ne!(bytes(&changed), expected, "participant order is retained");

    let mut changed = original.clone();
    changed.participants[0].role = Some("  rôle différent  ".into());
    assert_ne!(
        bytes(&changed),
        expected,
        "participant optional role is retained"
    );

    let mut changed = original.clone();
    changed.participants[1].role = Some("new optional role".into());
    assert_ne!(
        bytes(&changed),
        expected,
        "None and Some participant roles differ"
    );

    let mut changed = original.clone();
    changed.evidence[0].owner_id = "  autre mémoire  ".into();
    assert_ne!(bytes(&changed), expected, "evidence owner is retained");

    let mut changed = original.clone();
    changed.evidence[1].memory_id = MemoryId([0x55; 32]);
    assert_ne!(bytes(&changed), expected, "evidence identity is retained");

    let mut changed = original.clone();
    changed.evidence.swap(0, 1);
    assert_ne!(bytes(&changed), expected, "evidence order is retained");

    let mut changed = original.clone();
    changed.mutation_id = "  mutation changed  ".into();
    assert_ne!(bytes(&changed), expected, "mutation identity is retained");

    let mut changed = original.clone();
    changed.updated_at_ns += 1;
    assert_ne!(bytes(&changed), expected, "update time is retained");

    let mut changed = original;
    changed.created_at_ns += 1;
    assert_ne!(bytes(&changed), expected, "creation time is retained");
}
