use super::*;
use crate::entity_compact_contract::EntityDescriptionProof;
use crate::{EntityGlobalId, EntityId};
use reliquary_container::codec::FieldChange;

fn first() -> EntityRecord {
    EntityRecord {
        id: EntityId([1; 32]),
        global_id: None,
        revision: 1,
        canonical_name: "Name".into(),
        aliases: vec!["Alias".into()],
        kind: "project".into(),
        summary: "Summary".into(),
        mutation_id: "first".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 9,
        entity_version: 1,
    }
}

#[test]
fn every_field_combination_reconstructs_exactly_without_new_order_constraints() {
    for mask in 0..64 {
        let base = first();
        let mut next = base.clone();
        next.revision = 2;
        next.entity_version = 4;
        next.global_version = 3;
        next.mutation_id = "second".into();
        if mask & 1 != 0 {
            next.global_id = Some(EntityGlobalId([0; 16]));
        }
        if mask & 2 != 0 {
            next.canonical_name = "New name".into();
        }
        if mask & 4 != 0 {
            next.aliases = vec!["Another".into(), "Zed".into()];
        }
        if mask & 8 != 0 {
            next.kind = "person".into();
        }
        if mask & 16 != 0 {
            next.summary.clear();
        }
        if mask & 32 != 0 {
            next.updated_at_ns = 10;
        }
        let delta = EntityDelta::between(&base, &next).unwrap();
        assert_eq!(delta.fields.mask(), mask);
        assert_eq!(apply_delta(&base, &delta).unwrap(), next);
    }
}

#[test]
fn uuid_clear_and_replace_reconstruct_exactly() {
    let mut base = first();
    base.global_id = Some(EntityGlobalId([7; 16]));
    for uuid in [None, Some(EntityGlobalId([8; 16]))] {
        let mut next = base.clone();
        next.revision = 2;
        next.entity_version = 2;
        next.mutation_id = "next".into();
        next.global_id = uuid;
        assert_eq!(
            apply_delta(&base, &EntityDelta::between(&base, &next).unwrap()).unwrap(),
            next
        );
    }
}

#[test]
fn redundant_fields_and_wrong_base_are_rejected() {
    let base = first();
    let mut next = base.clone();
    next.revision = 2;
    next.entity_version = 2;
    next.mutation_id = "next".into();
    let mut delta = EntityDelta::between(&base, &next).unwrap();
    delta.fields.summary = FieldChange::Set(base.summary.clone());
    assert!(apply_delta(&base, &delta).is_err());
    let mut delta = EntityDelta::between(&base, &next).unwrap();
    delta.id = EntityId([2; 32]);
    assert!(apply_delta(&base, &delta).is_err());
    delta.id = base.id;
    delta.previous_revision = 0;
    assert!(apply_delta(&base, &delta).is_err());
    delta.previous_revision = 1;
    delta.entity_version = 1;
    assert!(apply_delta(&base, &delta).is_err());
}

#[test]
fn pruning_evidence_survives_candidate_validation_without_granting_authorization() {
    let base = first();
    let mut next = base.clone();
    next.revision = 2;
    next.entity_version = 2;
    next.mutation_id = "next".into();
    next.canonical_name = "[pruned]".into();
    next.aliases.clear();
    next.summary.clear();
    let proof = EntityDescriptionProof {
        original_carrier_digest: [3; 32],
        manifest_digest: [4; 32],
    };
    let delta = EntityDelta::between_pruned(&base, &next, proof.clone()).unwrap();
    assert_eq!(apply_delta(&base, &delta).unwrap(), next);
    assert_eq!(delta.description_proof, Some(proof));
    let mut invalid = delta;
    invalid.fields.summary = FieldChange::Set("plaintext".into());
    assert!(apply_delta(&base, &invalid).is_err());
}
