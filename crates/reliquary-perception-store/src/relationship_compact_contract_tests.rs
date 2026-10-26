//! Contract regressions for exact accepted Relationship history.
use super::*;
use crate::{EntityId, EntityRef, RelationshipId, RelationshipMemoryRef};

fn entity(owner_id: &str, id: u8, role: Option<&str>) -> RelationshipParticipant {
    RelationshipParticipant {
        entity: EntityRef {
            owner_id: owner_id.into(),
            entity_id: EntityId([id; 32]),
        },
        role: role.map(str::to_owned),
    }
}

fn evidence(owner_id: &str, id: u8) -> RelationshipMemoryRef {
    RelationshipMemoryRef {
        owner_id: owner_id.into(),
        memory_id: [id; 32],
    }
}

fn record() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x11; 32]),
        revision: 1,
        kind: "related_to".into(),
        participants: vec![
            entity("local", 1, Some("source")),
            entity("remote", 2, None),
        ],
        evidence: vec![evidence("local", 3), evidence("remote", 4)],
        summary: "description".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 8,
        relationship_version: 3,
    }
}

fn next_record(previous: &RelationshipRecord) -> RelationshipRecord {
    let mut next = previous.clone();
    next.revision += 1;
    next.global_version = 2; // Valid earlier late-bound global ticket.
    next.relationship_version += 2; // Interleaved with another Relationship ID.
    next.mutation_id = "mutation-2".into();
    next
}

#[test]
fn creation_preserves_exact_validated_record_values_without_draft_normalization() {
    let mut source = record();
    source.kind = "  relationship kind  ".into();
    source.summary = "  exact summary  ".into();
    source.mutation_id = "  mutation  ".into();
    source.participants[0].entity.owner_id = "  local-owner  ".into();
    source.participants[0].role = Some("  source role  ".into());
    source.evidence[0].owner_id = "  local-owner  ".into();

    let creation = RelationshipCreation::new(source.clone()).unwrap();
    assert_eq!(creation.record, source);
    assert_eq!(creation.record.revision, 1);
}

#[test]
fn delta_tracks_scalar_changes_and_exact_sorted_participant_evidence_set_ops() {
    let previous = record();
    let mut next = next_record(&previous);
    next.kind = "supports".into();
    next.summary = "changed".into();
    next.updated_at_ns = 15; // Backward relative to 20, but >= created_at_ns.

    // A participant role edit is represented as removal of the old exact tuple
    // and addition of the new tuple; it does not replace by EntityRef alone.
    next.participants.remove(0);
    next.participants
        .push(entity("local", 1, Some("contributor")));
    next.participants.sort();
    next.evidence.remove(0);
    next.evidence.push(evidence("local", 9));
    next.evidence.sort_by(|left, right| {
        left.owner_id
            .cmp(&right.owner_id)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });

    let delta = RelationshipDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 0b1_1111);
    assert_eq!(
        delta.fields.participants.removed,
        vec![entity("local", 1, Some("source"))]
    );
    assert_eq!(
        delta.fields.participants.added,
        vec![entity("local", 1, Some("contributor"))]
    );
    assert_eq!(delta.fields.evidence.removed, vec![evidence("local", 3)]);
    assert_eq!(delta.fields.evidence.added, vec![evidence("local", 9)]);
    delta.fields.validate_canonical().unwrap();
}

#[test]
fn empty_delta_is_legal_for_a_fresh_mutation_and_retains_late_global_ticket() {
    let previous = record();
    let next = next_record(&previous);
    let delta = RelationshipDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.global_version, 2);
    assert_eq!(delta.fields.mask(), 0);
}

#[test]
fn rejects_bad_revision_identity_clocks_mutation_and_created_time() {
    let previous = record();

    let mut bad = next_record(&previous);
    bad.id = RelationshipId([0x22; 32]);
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::InvalidField(
            "Relationship delta identity"
        ))
    ));

    let mut bad = next_record(&previous);
    bad.revision = 3;
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::RevisionConflict)
    ));

    let mut bad = next_record(&previous);
    bad.relationship_version = previous.relationship_version;
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::InvalidVersion)
    ));

    let mut bad = next_record(&previous);
    bad.global_version = 0;
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::InvalidVersion)
    ));

    let mut bad = next_record(&previous);
    bad.mutation_id = previous.mutation_id.clone();
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::MutationConflict)
    ));

    let mut bad = next_record(&previous);
    bad.created_at_ns += 1;
    assert!(matches!(
        RelationshipDelta::between(&previous, &bad),
        Err(RelationshipError::RevisionConflict)
    ));
}

#[test]
fn collection_ops_reject_noncanonical_duplicates_order_and_overlap() {
    let value_a = entity("a", 1, None);
    let value_b = entity("b", 2, None);
    let duplicate = RelationshipCollectionChange {
        removed: vec![value_a.clone(), value_a.clone()],
        added: vec![],
    };
    assert!(duplicate.validate_canonical().is_err());

    let unsorted = RelationshipCollectionChange {
        removed: vec![value_b.clone(), value_a.clone()],
        added: vec![],
    };
    assert!(unsorted.validate_canonical().is_err());

    let overlap = RelationshipCollectionChange {
        removed: vec![value_a.clone()],
        added: vec![value_a],
    };
    assert!(overlap.validate_canonical().is_err());

    assert!(RelationshipDelta::validate_mask(1 << 5).is_err());
}

#[test]
fn prune_proof_is_explicit_and_requires_canonical_empty_summary() {
    let proof = RelationshipDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let mut pruned = record();
    pruned.summary.clear();
    assert_eq!(
        RelationshipCreation::new(pruned.clone())
            .unwrap()
            .description_proof,
        None
    );
    assert_eq!(
        RelationshipCreation::new_pruned(pruned.clone(), proof.clone())
            .unwrap()
            .description_proof,
        Some(proof.clone())
    );

    let mut nonempty = pruned;
    nonempty.summary = "authored text".into();
    assert!(matches!(
        RelationshipCreation::new_pruned(nonempty, proof.clone()),
        Err(RelationshipError::CorruptRecord(
            "noncanonical pruned Relationship summary"
        ))
    ));

    let previous = record();
    let mut redacted_next = next_record(&previous);
    redacted_next.summary.clear();
    let delta =
        RelationshipDelta::between_pruned(&previous, &redacted_next, proof.clone()).unwrap();
    assert_eq!(delta.description_proof, Some(proof));
}
