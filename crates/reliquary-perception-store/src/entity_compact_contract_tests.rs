//! Owner contract tests independent of persisted framing.
//! Source replay behavior is separately characterized with actual V1 frames.
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

#[test]
fn field_order_and_every_reserved_mask_bit_are_explicit() {
    use super::{ENTITY_DELTA_FIELD_ORDER, ENTITY_DELTA_KNOWN_MASK};
    assert_eq!(
        ENTITY_DELTA_FIELD_ORDER,
        &[
            "global_id",
            "canonical_name",
            "aliases",
            "kind",
            "summary",
            "updated_at_ns"
        ]
    );
    assert_eq!(ENTITY_DELTA_KNOWN_MASK, 63);
    for mask in 0..=63 {
        assert!(EntityDelta::validate_mask(mask).is_ok());
    }
    for bit in 6..64 {
        assert!(EntityDelta::validate_mask(1_u64 << bit).is_err());
    }
}

#[test]
fn retained_nonprincipal_zero_id_and_uuid_are_not_newly_rejected() {
    let mut first = record();
    first.id = EntityId([0; 32]);
    first.global_id = Some(EntityGlobalId([0; 16]));
    assert_eq!(EntityCreation::new(first.clone()).unwrap().record, first);
}

#[test]
fn pruning_is_explicit_authenticated_metadata_not_placeholder_inference() {
    use super::EntityDescriptionProof;
    let mut redacted = record();
    redacted.canonical_name = "[pruned]".into();
    redacted.aliases.clear();
    redacted.summary.clear();
    assert_eq!(
        EntityCreation::new(redacted.clone())
            .unwrap()
            .description_proof,
        None
    );
    let proof = EntityDescriptionProof {
        original_carrier_digest: [7; 32],
        manifest_digest: [8; 32],
    };
    let pruned = EntityCreation::new_pruned(redacted.clone(), proof.clone()).unwrap();
    assert_eq!(pruned.record, redacted);
    assert_eq!(pruned.description_proof, Some(proof.clone()));
    let next = successor(&redacted, "pruned-next-mutation");
    let delta = EntityDelta::between_pruned(&redacted, &next, proof.clone()).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert_eq!(delta.description_proof, Some(proof.clone()));
    assert!(EntityCreation::new_pruned(record(), proof).is_err());
}

fn record() -> EntityRecord {
    EntityRecord {
        id: EntityId([0x61; 32]),
        global_id: None,
        revision: 1,
        canonical_name: "External referent".into(),
        aliases: vec!["Alternate name".into()],
        kind: "project".into(),
        summary: "A stable summary.".into(),
        mutation_id: "entity-mutation-1".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 2,
        entity_version: 1,
    }
}

fn successor(previous: &EntityRecord, mutation_id: &str) -> EntityRecord {
    let mut next = previous.clone();
    next.revision = previous.revision + 1;
    // Entity versions are owner-local and may have gaps due to other IDs.
    next.entity_version = previous.entity_version + 2;
    // A successor may bind an earlier already-reserved global ticket.
    next.global_version = 1;
    next.mutation_id = mutation_id.into();
    next
}

#[test]
fn creation_retains_legacy_missing_global_id_and_exact_first_record() {
    let first = record();
    assert_eq!(first.global_id, None);
    let creation = EntityCreation::new(first.clone()).unwrap();
    assert_eq!(creation.record, first);
}

#[test]
fn global_id_none_some_replacement_and_clear_are_representable_changes() {
    let first = record();
    let mut second = successor(&first, "entity-mutation-2");
    second.global_id = Some(EntityGlobalId([0xA1; 16]));
    let promotion = EntityDelta::between(&first, &second).unwrap();
    assert_eq!(promotion.fields.mask(), 1 << 0);
    assert_eq!(promotion.global_version, 1);
    assert_eq!(promotion.entity_version, 3);

    let mut third = successor(&second, "entity-mutation-3");
    third.global_id = Some(EntityGlobalId([0xB2; 16]));
    third.global_version = 3;
    let replacement = EntityDelta::between(&second, &third).unwrap();
    assert_eq!(replacement.fields.mask(), 1 << 0);

    let mut fourth = successor(&third, "entity-mutation-4");
    fourth.global_id = None;
    fourth.global_version = 4;
    let clear = EntityDelta::between(&third, &fourth).unwrap();
    assert_eq!(clear.fields.mask(), 1 << 0);
}

#[test]
fn fresh_mutation_with_no_changed_fields_has_zero_mask() {
    let first = record();
    let mut next = successor(&first, "entity-mutation-2");
    next.entity_version = first.entity_version + 1;
    next.global_version = 1;
    let delta = EntityDelta::between(&first, &next).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert_eq!(delta.mutation_id, "entity-mutation-2");
}

#[test]
fn changed_aliases_are_one_exact_ordered_replacement() {
    let first = record();
    let mut next = successor(&first, "entity-mutation-2");
    next.aliases = vec!["A name".into(), "Z name".into()];
    let delta = EntityDelta::between(&first, &next).unwrap();
    assert_eq!(delta.fields.mask(), 1 << 2);
    assert_eq!(next.aliases, vec!["A name", "Z name"]);
}

#[test]
fn updated_at_may_move_backward_while_staying_after_creation() {
    let first = record();
    let mut next = successor(&first, "entity-mutation-2");
    next.updated_at_ns = 11;
    let delta = EntityDelta::between(&first, &next).unwrap();
    assert_eq!(delta.fields.mask(), 1 << 5);
    assert!(next.updated_at_ns >= next.created_at_ns);
}

#[test]
fn distinct_nonempty_mutation_and_advancing_local_clock_are_required() {
    let first = record();

    let mut unchanged_mutation = successor(&first, "unused");
    unchanged_mutation.mutation_id = first.mutation_id.clone();
    assert!(EntityDelta::between(&first, &unchanged_mutation).is_err());

    let mut empty_mutation = successor(&first, "unused");
    empty_mutation.mutation_id.clear();
    assert!(EntityDelta::between(&first, &empty_mutation).is_err());

    let mut zero_local = successor(&first, "entity-mutation-2");
    zero_local.entity_version = 0;
    assert!(EntityDelta::between(&first, &zero_local).is_err());

    let mut same_local = successor(&first, "entity-mutation-3");
    same_local.entity_version = first.entity_version;
    assert!(EntityDelta::between(&first, &same_local).is_err());

    let mut zero_global = successor(&first, "entity-mutation-4");
    zero_global.global_version = 0;
    assert!(EntityDelta::between(&first, &zero_global).is_err());
}

#[test]
fn immutable_identity_creation_time_and_exact_revision_successor_are_required() {
    let first = record();

    let mut changed_id = successor(&first, "entity-mutation-2");
    changed_id.id = EntityId([0x62; 32]);
    assert!(EntityDelta::between(&first, &changed_id).is_err());

    let mut changed_creation_time = successor(&first, "entity-mutation-3");
    changed_creation_time.created_at_ns += 1;
    assert!(EntityDelta::between(&first, &changed_creation_time).is_err());

    let mut skipped_revision = successor(&first, "entity-mutation-4");
    skipped_revision.revision += 1;
    assert!(EntityDelta::between(&first, &skipped_revision).is_err());

    let mut zero_revision = successor(&first, "entity-mutation-5");
    zero_revision.revision = 0;
    assert!(EntityDelta::between(&first, &zero_revision).is_err());
}

#[test]
fn creation_rejects_zero_owner_coordinates_and_invalid_timestamps() {
    let mut zero_global = record();
    zero_global.global_version = 0;
    assert!(EntityCreation::new(zero_global).is_err());

    let mut zero_local = record();
    zero_local.entity_version = 0;
    assert!(EntityCreation::new(zero_local).is_err());

    let mut invalid_time = record();
    invalid_time.updated_at_ns = invalid_time.created_at_ns - 1;
    assert!(EntityCreation::new(invalid_time).is_err());

    let mut invalid_alias_order = record();
    invalid_alias_order.aliases = vec!["Z name".into(), "A name".into()];
    assert!(EntityCreation::new(invalid_alias_order).is_err());

    let mut too_long_name = record();
    too_long_name.canonical_name = "x".repeat(crate::entity_model::MAX_ENTITY_NAME_BYTES + 1);
    assert!(EntityCreation::new(too_long_name).is_err());

    let mut bad_principal = record();
    bad_principal.kind = crate::entity_identity::PRINCIPAL_ENTITY_KIND.into();
    bad_principal.canonical_name = "phy-00000000-0000-0000-0000-0000000000a1".into();
    bad_principal.id = EntityId([0x61; 32]);
    assert!(EntityCreation::new(bad_principal).is_err());
}
