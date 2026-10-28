use reliquary_entity_resolution::{
    CandidateEvidence, candidate_order, creation_conflicts, finalize_candidate,
    reconciliation_candidates,
};
use reliquary_memory_core::MemoryId;
use reliquary_perception_store::{Entity, EntityId};

fn entity(id: u8, name: &str, kind: &str) -> Entity {
    Entity {
        id: EntityId([id; 32]),
        global_id: None,
        revision: 1,
        canonical_name: name.into(),
        aliases: Vec::new(),
        kind: kind.into(),
        summary: String::new(),
        mutation_id: String::new(),
        created_at_ns: 0,
        updated_at_ns: 0,
        global_version: 1,
        entity_version: 1,
    }
}

#[test]
fn ranking_is_stable_and_support_is_bounded() {
    let mut evidence = CandidateEvidence::default();
    evidence.lexical_memories.insert(MemoryId([2; 32]), 0.8);
    evidence.lexical_memories.insert(MemoryId([1; 32]), 0.8);
    evidence.graph_memories.insert(MemoryId([3; 32]));
    let first = finalize_candidate(entity(1, "Alpha", "tool"), evidence, 2);
    assert_eq!(
        first.supporting_memory_ids,
        vec![MemoryId([1; 32]), MemoryId([2; 32])]
    );
    assert_eq!(
        (first.lexical_memory_hits, first.graph_neighbor_hits),
        (2, 1)
    );
    let second = finalize_candidate(entity(2, "Beta", "tool"), CandidateEvidence::default(), 2);
    assert!(candidate_order(&first, &second).is_lt());
    let tie = finalize_candidate(entity(3, "Gamma", "tool"), CandidateEvidence::default(), 2);
    assert!(candidate_order(&second, &tie).is_lt());
}

#[test]
fn snapshot_conflicts_and_reconciliation_preserve_distinctions() {
    let mut known = entity(2, "PlayerHuePresenter", "code_type");
    known.aliases.push("presenter".into());
    let distinct = entity(3, "Player Hue Presenter", "tool");
    let principal = entity(4, "Player Hue Presenter", "principal");
    let values = vec![known.clone(), distinct, principal];
    let before = values.clone();
    assert_eq!(
        creation_conflicts(&values, "Player Hue Presenter", "code_type", 8),
        vec![known]
    );
    assert!(creation_conflicts(&values, "Player Hue Presenter", "principal", 8).is_empty());
    assert!(
        reconciliation_candidates(&values, 16)
            .iter()
            .all(|pair| pair.left != EntityId([4; 32]) && pair.right != EntityId([4; 32]))
    );
    assert_eq!(values, before);
}
