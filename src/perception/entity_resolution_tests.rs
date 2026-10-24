use crate::entity_resolution_test_support::{phy_with_three_mentions, rel_with_three_mentions};
use crate::{
    EntityId, EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryError, MemoryTextField, Phylactery,
};

#[test]
fn mixed_resolution_states_coexist_and_reopen() {
    let (mut rel, memory_id, keys, path) = rel_with_three_mentions();
    rel.put_entity_resolved(
        keys[0],
        0,
        EntityId([1; 32]),
        EntityResolutionReason::ContextMatch,
        10,
    )
    .unwrap();
    rel.put_entity_unresolved(
        keys[1],
        0,
        vec![EntityId([2; 32])],
        EntityResolutionReason::Ambiguous,
        [3; 32],
        [4; 32],
        10,
    )
    .unwrap();
    rel.put_entity_rejected(keys[2], 0, EntityResolutionReason::GenericRole, 10)
        .unwrap();

    let states = rel.entity_resolutions_for_memory(memory_id);
    assert_eq!(states.len(), 3);
    assert!(matches!(
        states[0].status,
        MemoryEntityResolutionStatus::Resolved { .. }
    ));
    assert!(matches!(
        states[1].status,
        MemoryEntityResolutionStatus::Pending(_)
    ));
    assert!(matches!(
        states[2].status,
        MemoryEntityResolutionStatus::Rejected { .. }
    ));
    rel.sync().unwrap();
    drop(rel);

    let reopened = crate::Cva::open(&path).unwrap();
    assert_eq!(reopened.entity_resolutions_for_memory(memory_id), states);
}

#[test]
fn pending_retries_only_when_evidence_changes() {
    let (mut rel, _, keys, _) = rel_with_three_mentions();
    let key = keys[0];
    rel.put_entity_unresolved(
        key,
        0,
        vec![EntityId([7; 32])],
        EntityResolutionReason::InsufficientEvidence,
        [1; 32],
        [2; 32],
        100,
    )
    .unwrap();

    assert!(!rel.entity_resolution_retry_needed(key, [1; 32], [2; 32]));
    assert!(rel.entity_resolution_retry_needed(key, [9; 32], [2; 32]));
    assert!(
        !rel.put_entity_unresolved(
            key,
            1,
            vec![EntityId([8; 32])],
            EntityResolutionReason::Ambiguous,
            [1; 32],
            [2; 32],
            200,
        )
        .unwrap()
    );

    rel.put_entity_unresolved(
        key,
        1,
        vec![EntityId([8; 32]), EntityId([7; 32]), EntityId([7; 32])],
        EntityResolutionReason::Ambiguous,
        [9; 32],
        [2; 32],
        200,
    )
    .unwrap();
    let value = rel.entity_resolution(key).unwrap();
    assert_eq!(value.revision, 2);
    let MemoryEntityResolutionStatus::Pending(pending) = &value.status else {
        panic!("expected pending");
    };
    assert_eq!(pending.attempt_count, 2);
    assert_eq!(
        pending.candidate_entity_ids,
        vec![EntityId([7; 32]), EntityId([8; 32])]
    );
}

#[test]
fn exact_mention_key_and_candidate_limit_are_enforced() {
    let (mut rel, memory_id, keys, _) = rel_with_three_mentions();
    let bad = MemoryEntityMentionKey {
        memory_id,
        field: MemoryTextField::Content,
        start_byte: keys[0].start_byte,
        end_byte: keys[0].end_byte + 1,
    };
    assert!(matches!(
        rel.put_entity_rejected(bad, 0, EntityResolutionReason::GenericRole, 1),
        Err(MemoryError::InvalidField("Entity resolution mention"))
    ));

    let candidates = (0..=crate::MAX_ENTITY_RESOLUTION_CANDIDATES)
        .map(|value| EntityId([value as u8; 32]))
        .collect();
    assert!(matches!(
        rel.put_entity_unresolved(
            keys[0],
            0,
            candidates,
            EntityResolutionReason::Ambiguous,
            [1; 32],
            [2; 32],
            1,
        ),
        Err(MemoryError::FieldTooLarge)
    ));
}

#[test]
fn phylactery_resolution_state_reopens() {
    let (mut phy, memory_id, keys, path) = phy_with_three_mentions();
    phy.put_entity_unresolved(
        keys[0],
        0,
        vec![],
        EntityResolutionReason::InsufficientEvidence,
        [1; 32],
        [2; 32],
        5,
    )
    .unwrap();
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.entity_resolutions_for_memory(memory_id).len(), 1);
}

#[test]
fn terminal_state_can_be_explicitly_reset_for_manual_correction() {
    let (mut rel, _, keys, _) = rel_with_three_mentions();
    let key = keys[0];
    rel.put_entity_rejected(key, 0, EntityResolutionReason::GenericRole, 1)
        .unwrap();

    assert!(rel.reset_entity_resolution(key, 2).unwrap());
    assert!(rel.entity_resolution(key).is_none());
    rel.put_entity_resolved(
        key,
        0,
        EntityId([1; 32]),
        EntityResolutionReason::ContextMatch,
        3,
    )
    .unwrap();
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Resolved { .. }
    ));
}

#[test]
fn terminal_states_are_not_silently_reopened() {
    let (mut rel, _, keys, _) = rel_with_three_mentions();
    rel.put_entity_resolved(
        keys[0],
        0,
        EntityId([1; 32]),
        EntityResolutionReason::ContextMatch,
        1,
    )
    .unwrap();
    assert!(matches!(
        rel.put_entity_unresolved(
            keys[0],
            1,
            vec![],
            EntityResolutionReason::Ambiguous,
            [1; 32],
            [2; 32],
            2,
        ),
        Err(MemoryError::InvalidField("terminal Entity resolution"))
    ));
}

#[test]
fn entity_resolution_owner_accepts_typed_target_but_owner_admission_rejects_missing_entity_atomically()
 {
    let (mut direct, _, keys, _) = rel_with_three_mentions();
    let key = keys[0];
    let missing_entity = EntityId([250; 32]);
    direct
        .owner
        .entity_resolutions
        .put_resolved(
            &mut direct.owner.container,
            key,
            0,
            missing_entity,
            EntityResolutionReason::ContextMatch,
            10,
        )
        .unwrap();
    assert!(matches!(
        direct.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == missing_entity
    ));

    let (mut composed, _, keys, _) = rel_with_three_mentions();
    let before = composed.container.latest_version();
    assert!(matches!(
        composed.put_entity_resolved(
            keys[0],
            0,
            missing_entity,
            EntityResolutionReason::ContextMatch,
            10,
        ),
        Err(MemoryError::InvalidField(
            "Entity resolution Entity reference"
        ))
    ));
    assert_eq!(composed.container.latest_version(), before);
    assert!(composed.entity_resolution(keys[0]).is_none());
}
