use crate::entity_candidate_test_support::entity_draft;
use crate::entity_reconciliation_candidates::reconciliation_candidates;
use crate::{Entity, EntityGlobalId, EntityId};

fn entity(name: &str, kind: &str, seed: u8) -> Entity {
    let draft = entity_draft(name, &[], "summary", seed as i64);
    Entity {
        id: EntityId([seed; 32]),
        global_id: Some(EntityGlobalId([seed; 16])),
        revision: 1,
        canonical_name: draft.canonical_name,
        aliases: draft.aliases,
        kind: kind.into(),
        summary: draft.summary,
        mutation_id: draft.mutation_id,
        created_at_ns: draft.created_at_ns,
        updated_at_ns: draft.updated_at_ns,
        global_version: 1,
        entity_version: 1,
    }
}

#[test]
fn finds_short_name_and_shared_anchor_pairs() {
    let entities = vec![
        entity("Eastwood", "domain_entity", 1),
        entity("Eastwood Middle", "domain_entity", 2),
        entity("family garden", "domain_entity", 3),
        entity("backyard garden", "domain_entity", 4),
    ];
    let pairs = reconciliation_candidates(&entities, 16);
    assert!(
        pairs
            .iter()
            .any(|pair| { pair.left == EntityId([1; 32]) && pair.right == EntityId([2; 32]) })
    );
    assert!(
        pairs
            .iter()
            .any(|pair| { pair.left == EntityId([3; 32]) && pair.right == EntityId([4; 32]) })
    );
}

#[test]
fn principal_entities_never_enter_reconciliation_candidates() {
    let entities = vec![
        entity("Shared Identity", "principal", 10),
        entity("Shared Identity", "principal", 11),
        entity("Shared Identity", "domain_entity", 12),
    ];
    let pairs = reconciliation_candidates(&entities, 16);
    assert!(pairs.iter().all(|pair| pair.left != EntityId([10; 32])
        && pair.right != EntityId([10; 32])
        && pair.left != EntityId([11; 32])
        && pair.right != EntityId([11; 32])));
}
