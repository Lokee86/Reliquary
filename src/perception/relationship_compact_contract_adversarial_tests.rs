//! Additional draft adversarial tests; include in the future contract test module.
use crate::relationship_compact_contract::{
    RELATIONSHIP_DELTA_KNOWN_MASK, RelationshipCollectionChange, RelationshipCreation,
    RelationshipDelta,
};
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{
    EntityId, EntityRef, MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES,
    MAX_RELATIONSHIP_OWNER_ID_BYTES, MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES,
    MAX_RELATIONSHIP_SUMMARY_BYTES, MemoryId, MemoryRef, RelationshipError, RelationshipId,
};
use reliquary_container::codec::FieldChange;

fn participant(owner: &str, id: u32, role: Option<&str>) -> RelationshipParticipant {
    let mut raw = [0; 32];
    raw[..4].copy_from_slice(&id.to_be_bytes());
    RelationshipParticipant {
        entity: EntityRef {
            owner_id: owner.into(),
            entity_id: EntityId(raw),
        },
        role: role.map(str::to_owned),
    }
}

fn memory(owner: &str, id: u32) -> MemoryRef {
    let mut raw = [0; 32];
    raw[..4].copy_from_slice(&id.to_be_bytes());
    MemoryRef {
        owner_id: owner.into(),
        memory_id: MemoryId(raw),
    }
}

fn base() -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x41; 32]),
        revision: 1,
        kind: "related_to".into(),
        participants: vec![
            participant("local", 1, Some("source")),
            participant("remote", 2, None),
        ],
        evidence: vec![memory("local", 3), memory("remote", 4)],
        summary: "summary".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 9,
        relationship_version: 4,
    }
}

fn successor(previous: &RelationshipRecord) -> RelationshipRecord {
    let mut next = previous.clone();
    next.revision += 1;
    next.global_version = 2; // The accepted owner contract does not require increasing tickets.
    next.relationship_version += 2; // One intervening owner-local Relationship publication.
    next.mutation_id = format!("mutation-{}", next.revision);
    next
}

#[test]
fn every_mutable_field_mask_derives_exactly_and_role_option_transitions_are_set_ops() {
    let previous = base();
    for mask in 0..=RELATIONSHIP_DELTA_KNOWN_MASK {
        let mut next = successor(&previous);
        if mask & (1 << 0) != 0 {
            next.kind = "supports".into();
        }
        if mask & (1 << 1) != 0 {
            next.participants.remove(0); // role Some("source")
            next.participants.push(participant("local", 1, None));
            next.participants.sort();
        }
        if mask & (1 << 2) != 0 {
            next.evidence.remove(0);
            next.evidence.push(memory("local", 8));
            next.evidence.sort_by(|a, b| {
                a.owner_id
                    .cmp(&b.owner_id)
                    .then_with(|| a.memory_id.0.cmp(&b.memory_id.0))
            });
        }
        if mask & (1 << 3) != 0 {
            next.summary = "changed summary".into();
        }
        if mask & (1 << 4) != 0 {
            next.updated_at_ns = 15;
        }
        let delta = RelationshipDelta::between(&previous, &next).unwrap();
        assert_eq!(delta.fields.mask(), mask, "mask {mask:#07b}");
        delta.fields.validate_canonical().unwrap();
        if mask & (1 << 1) != 0 {
            assert_eq!(
                delta.fields.participants.removed,
                vec![participant("local", 1, Some("source"))]
            );
            assert_eq!(
                delta.fields.participants.added,
                vec![participant("local", 1, None)]
            );
        }
    }

    let mut role_some = successor(&previous);
    role_some.participants[0].role = None;
    let first = RelationshipDelta::between(&previous, &role_some).unwrap();
    assert_eq!(
        first.fields.participants.added,
        vec![participant("local", 1, None)]
    );
    let mut role_replaced = successor(&role_some);
    role_replaced.relationship_version += 1;
    role_replaced.participants[0].role = Some("target".into());
    let second = RelationshipDelta::between(&role_some, &role_replaced).unwrap();
    assert_eq!(
        second.fields.participants.removed,
        vec![participant("local", 1, None)]
    );
    assert_eq!(
        second.fields.participants.added,
        vec![participant("local", 1, Some("target"))]
    );
}

#[test]
fn record_collection_limits_and_empty_required_participants_match_v1_admission() {
    let mut empty = base();
    empty.participants.clear();
    assert!(matches!(
        RelationshipCreation::new(empty),
        Err(RelationshipError::InvalidField("Relationship participants"))
    ));

    let mut max_participants = base();
    max_participants.participants = (0..MAX_RELATIONSHIP_PARTICIPANTS as u32)
        .map(|id| participant("owner", id, None))
        .collect();
    assert!(RelationshipCreation::new(max_participants.clone()).is_ok());
    max_participants.participants.push(participant(
        "owner",
        MAX_RELATIONSHIP_PARTICIPANTS as u32,
        None,
    ));
    assert!(matches!(
        RelationshipCreation::new(max_participants),
        Err(RelationshipError::InvalidField("Relationship participants"))
    ));

    let mut max_evidence = base();
    max_evidence.evidence = (0..MAX_RELATIONSHIP_EVIDENCE as u32)
        .map(|id| memory("owner", id))
        .collect();
    assert!(RelationshipCreation::new(max_evidence.clone()).is_ok());
    max_evidence
        .evidence
        .push(memory("owner", MAX_RELATIONSHIP_EVIDENCE as u32));
    assert!(matches!(
        RelationshipCreation::new(max_evidence),
        Err(RelationshipError::InvalidField("Relationship evidence"))
    ));
}

#[test]
fn collection_delta_limits_are_bounded_per_sorted_operation_list() {
    let participant_removals = (0..=MAX_RELATIONSHIP_PARTICIPANTS as u32)
        .map(|id| participant("owner", id, None))
        .collect::<Vec<_>>();
    let too_many_participants = RelationshipCollectionChange {
        removed: participant_removals,
        added: vec![],
    };
    let mut fields = RelationshipDelta::between(&base(), &successor(&base()))
        .unwrap()
        .fields;
    fields.participants = too_many_participants;
    assert!(fields.validate_canonical().is_err());

    let evidence_additions = (0..=MAX_RELATIONSHIP_EVIDENCE as u32)
        .map(|id| memory("owner", id))
        .collect::<Vec<_>>();
    let too_many_evidence = RelationshipCollectionChange {
        removed: vec![],
        added: evidence_additions,
    };
    let mut fields = RelationshipDelta::between(&base(), &successor(&base()))
        .unwrap()
        .fields;
    fields.evidence = too_many_evidence;
    assert!(fields.validate_canonical().is_err());
}

#[test]
fn invalid_operation_order_duplicates_and_reserved_bits_reject() {
    let first = participant("a", 1, None);
    let second = participant("b", 2, None);
    assert!(
        RelationshipCollectionChange {
            removed: vec![second.clone(), first.clone()],
            added: vec![],
        }
        .validate_canonical()
        .is_err()
    );
    assert!(
        RelationshipCollectionChange {
            removed: vec![first.clone(), first.clone()],
            added: vec![],
        }
        .validate_canonical()
        .is_err()
    );
    assert!(
        RelationshipCollectionChange {
            removed: vec![first.clone()],
            added: vec![first],
        }
        .validate_canonical()
        .is_err()
    );

    let e1 = memory("a", 1);
    let e2 = memory("b", 2);
    assert!(
        RelationshipCollectionChange {
            removed: vec![e2, e1],
            added: vec![],
        }
        .validate_canonical()
        .is_err()
    );
    assert!(RelationshipDelta::validate_mask(1 << 5).is_err());
}

#[test]
fn limits_are_utf8_byte_limits_and_owner_validation_stays_exact() {
    let mut valid = base();
    valid.participants.truncate(1);
    valid.evidence.truncate(1);
    valid.kind = "é".repeat(MAX_RELATIONSHIP_KIND_BYTES / 2);
    valid.participants[0].role = Some("é".repeat(MAX_RELATIONSHIP_ROLE_BYTES / 2));
    valid.participants[0].entity.owner_id = "é".repeat(MAX_RELATIONSHIP_OWNER_ID_BYTES / 2);
    valid.evidence[0].owner_id = "é".repeat(MAX_RELATIONSHIP_OWNER_ID_BYTES / 2);
    valid.summary = "é".repeat(MAX_RELATIONSHIP_SUMMARY_BYTES / 2);
    assert!(RelationshipCreation::new(valid.clone()).is_ok());
    assert_eq!(valid.kind.len(), MAX_RELATIONSHIP_KIND_BYTES);
    assert_eq!(valid.summary.len(), MAX_RELATIONSHIP_SUMMARY_BYTES);

    let mut over_kind = valid.clone();
    over_kind.kind.push('é');
    assert!(RelationshipCreation::new(over_kind).is_err());
    let mut over_role = valid.clone();
    over_role.participants[0].role.as_mut().unwrap().push('é');
    assert!(RelationshipCreation::new(over_role).is_err());
    let mut over_owner = valid.clone();
    over_owner.participants[0].entity.owner_id.push('é');
    assert!(RelationshipCreation::new(over_owner).is_err());
    let mut over_summary = valid;
    over_summary.summary.push('é');
    assert!(RelationshipCreation::new(over_summary).is_err());
}

#[test]
fn update_time_can_move_backward_but_not_before_creation_and_revision_overflow_rejects() {
    let previous = base();
    let mut backward = successor(&previous);
    backward.updated_at_ns = 10;
    assert!(RelationshipDelta::between(&previous, &backward).is_ok());
    backward.updated_at_ns = 9;
    assert!(RelationshipDelta::between(&previous, &backward).is_err());

    let mut max_revision = previous.clone();
    max_revision.revision = u64::MAX;
    let mut impossible = max_revision.clone();
    impossible.revision = 1;
    impossible.relationship_version += 1;
    impossible.mutation_id = "overflow-successor".into();
    assert!(matches!(
        RelationshipDelta::between(&max_revision, &impossible),
        Err(RelationshipError::VersionExhausted)
    ));
}

#[test]
fn authored_empty_summary_is_distinct_from_explicitly_pruned_summary() {
    let proof = crate::relationship_compact_contract::RelationshipDescriptionProof {
        original_carrier_digest: [0xA5; 32],
        manifest_digest: [0x5A; 32],
    };
    let mut empty = base();
    empty.summary.clear();
    let authored = RelationshipCreation::new(empty.clone()).unwrap();
    let pruned = RelationshipCreation::new_pruned(empty, proof.clone()).unwrap();
    assert_eq!(authored.record.summary, pruned.record.summary);
    assert_eq!(authored.description_proof, None);
    assert_eq!(pruned.description_proof, Some(proof));
}

#[test]
fn current_accepted_delta_does_not_conflate_mutation_identity_with_field_change() {
    let previous = base();
    let next = successor(&previous);
    let delta = RelationshipDelta::between(&previous, &next).unwrap();
    assert_eq!(delta.fields.mask(), 0);
    assert!(matches!(delta.fields.kind, FieldChange::Unchanged));
    assert!(delta.fields.participants.is_empty());
    assert!(delta.fields.evidence.is_empty());
}
