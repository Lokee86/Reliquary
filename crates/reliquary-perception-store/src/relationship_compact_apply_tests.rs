//! Pure Relationship candidate reconstruction regressions.
use crate::relationship_compact_apply::apply_delta;
use crate::relationship_compact_contract::{
    RelationshipCollectionChange, RelationshipCreation, RelationshipDelta,
    RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::{
    EntityId, EntityRef, RelationshipError, RelationshipId, RelationshipMemoryRef,
    RelationshipParticipant,
};
use reliquary_container::codec::FieldChange;

fn participant(owner: &str, id: u8, role: Option<&str>) -> RelationshipParticipant {
    RelationshipParticipant {
        entity: EntityRef {
            owner_id: owner.into(),
            entity_id: EntityId([id; 32]),
        },
        role: role.map(str::to_owned),
    }
}

fn evidence(owner: &str, id: u8) -> RelationshipMemoryRef {
    RelationshipMemoryRef {
        owner_id: owner.into(),
        memory_id: [id; 32],
    }
}

fn base() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x31; 32]),
        revision: 1,
        kind: "  observes  ".into(),
        participants: vec![
            participant("external-a", 1, Some("  source  ")),
            participant("external-b", 2, None),
        ],
        evidence: vec![evidence("memory-a", 3), evidence("memory-b", 4)],
        summary: "  authored description  ".into(),
        mutation_id: "initial".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 8,
        relationship_version: 3,
    }
}

fn next(previous: &RelationshipRecord) -> RelationshipRecord {
    let mut value = previous.clone();
    value.revision += 1;
    value.global_version = 2; // Legal late-bound ticket.
    value.relationship_version += 1;
    value.mutation_id = "next-mutation".into();
    value
}

#[test]
fn all_32_mutable_field_masks_reconstruct_the_exact_candidate() {
    let previous = base();
    for mask in 0_u64..32 {
        let mut candidate = next(&previous);
        if mask & 1 != 0 {
            candidate.kind = "  supports  ".into();
        }
        if mask & 2 != 0 {
            candidate.participants[0].role = Some("  contributor  ".into());
            candidate.participants.sort();
        }
        if mask & 4 != 0 {
            candidate.evidence[1] = evidence("memory-z", 9);
            candidate.evidence.sort_by(|left, right| {
                left.owner_id
                    .cmp(&right.owner_id)
                    .then_with(|| left.memory_id.cmp(&right.memory_id))
            });
        }
        if mask & 8 != 0 {
            candidate.summary = "  revised description  ".into();
        }
        if mask & 16 != 0 {
            candidate.updated_at_ns = 15; // A valid backward timestamp (>= creation).
        }
        let delta = RelationshipDelta::between(&previous, &candidate).unwrap();
        assert_eq!(delta.fields.mask(), mask, "field mask {mask:#07b}");
        assert_eq!(
            apply_delta(&previous, &delta).unwrap(),
            candidate,
            "field mask {mask:#07b}"
        );
    }
}

#[test]
fn malformed_collection_operations_and_redundant_sets_are_rejected() {
    let previous = base();
    let valid_next = next(&previous);
    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();

    delta.fields.participants = RelationshipCollectionChange {
        removed: vec![participant("absent", 7, None)],
        added: vec![],
    };
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "removal of absent tuple"
    );

    delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    let same = previous.participants[0].clone();
    delta.fields.participants = RelationshipCollectionChange {
        removed: vec![same.clone()],
        added: vec![same],
    };
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "overlapping remove/add"
    );

    delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.fields.participants = RelationshipCollectionChange {
        removed: vec![],
        added: vec![previous.participants[0].clone()],
    };
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "addition already in base"
    );

    delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    let absent = participant("new-owner", 8, None);
    delta.fields.participants = RelationshipCollectionChange {
        removed: vec![],
        added: vec![absent.clone(), absent],
    };
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "duplicate additions"
    );

    delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.fields.kind = FieldChange::Set(previous.kind.clone());
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "redundant scalar Set"
    );
}

#[test]
fn wrong_envelope_and_invalid_mutation_or_timestamp_are_rejected() {
    let previous = base();
    let valid_next = next(&previous);
    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.id = RelationshipId([0x99; 32]);
    assert!(matches!(
        apply_delta(&previous, &delta),
        Err(RelationshipError::InvalidField(_))
    ));

    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.previous_revision = 0;
    assert!(apply_delta(&previous, &delta).is_err());

    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.relationship_version = previous.relationship_version;
    assert!(apply_delta(&previous, &delta).is_err());

    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.mutation_id = previous.mutation_id.clone();
    assert!(matches!(
        apply_delta(&previous, &delta),
        Err(RelationshipError::MutationConflict)
    ));

    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.mutation_id.clear();
    assert!(apply_delta(&previous, &delta).is_err());

    let mut delta = RelationshipDelta::between(&previous, &valid_next).unwrap();
    delta.fields.updated_at_ns = FieldChange::Set(9);
    assert!(
        apply_delta(&previous, &delta).is_err(),
        "updated_at precedes immutable creation time"
    );
    assert!(
        RelationshipDelta::validate_mask(1 << 5).is_err(),
        "reserved mask bit"
    );
}

#[test]
fn explicit_proof_can_be_the_only_delta_payload_when_summary_is_already_empty() {
    let mut previous = base();
    previous.summary.clear();
    let candidate = next(&previous);
    let proof = RelationshipDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let delta = RelationshipDelta::between_pruned(&previous, &candidate, proof.clone()).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert_eq!(delta.description_proof, Some(proof));
    assert_eq!(apply_delta(&previous, &delta).unwrap(), candidate);

    let mut authored_empty = next(&candidate);
    authored_empty.mutation_id = "authored-empty-summary".into();
    let ordinary = RelationshipDelta::between(&candidate, &authored_empty).unwrap();
    assert_eq!(ordinary.description_proof, None);
    assert_eq!(apply_delta(&candidate, &ordinary).unwrap(), authored_empty);
    authored_empty.revision = 1;
    assert!(
        RelationshipCreation::new_pruned(
            authored_empty,
            RelationshipDescriptionProof {
                original_carrier_digest: [1; 32],
                manifest_digest: [2; 32],
            }
        )
        .is_ok()
    );
}
