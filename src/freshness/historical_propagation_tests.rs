use super::*;
use crate::{
    CommunityId,
    freshness::{PropagationEvent, propagate},
};
fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn community(n: u8) -> CommunityId {
    CommunityId([n; 32])
}
fn graph(edges: &[(u8, u8)]) -> MemoryAdjacency {
    let mut g = MemoryAdjacency::new();
    for &(a, b) in edges {
        g.entry(id(a)).or_default().insert(id(b));
    }
    g
}
fn event(kind: HistoricalPropagationKind, roots: &[(u8, i16)]) -> HistoricalPropagationEvent {
    HistoricalPropagationEvent {
        kind,
        position: 10,
        roots: roots.iter().map(|&(n, b)| (id(n), b)).collect(),
        credit_excluded: BTreeSet::new(),
    }
}
fn run(
    events: &[HistoricalPropagationEvent],
    g: &MemoryAdjacency,
    m: &CommunityMembership,
    authorized: &BTreeSet<MemoryId>,
    a: &BTreeMap<MemoryId, MemoryAvailability>,
    workers: usize,
) -> Vec<PropagationOutcome> {
    propagate_historical_events(
        events,
        g,
        m,
        authorized,
        a,
        workers,
        &FreshnessPolicy::default_v1(),
    )
    .unwrap()
}
fn available(nodes: &[u8]) -> BTreeMap<MemoryId, MemoryAvailability> {
    nodes
        .iter()
        .map(|&n| (id(n), MemoryAvailability::Supported { position: 1 }))
        .collect()
}

#[test]
fn historical_future_and_excluded_bridges_receive_no_credit_but_expand() {
    let g = graph(&[(1, 2), (2, 3), (3, 4), (4, 2)]);
    let mut a = available(&[1, 2, 3, 4]);
    a.insert(id(2), MemoryAvailability::Supported { position: 11 });
    let authorized = a.keys().copied().collect();
    let m = (1..=4).map(|n| (id(n), community(1))).collect();
    let mut e = event(HistoricalPropagationKind::FirstCycleDream, &[(1, 50)]);
    e.credit_excluded.insert(id(3));
    let out = run(&[e.clone()], &g, &m, &authorized, &a, 1).remove(0);
    assert_eq!(
        out.candidates,
        [(id(1), 50), (id(4), 35)].into_iter().collect()
    );
    assert!(out.stats.examined_edges >= 4);
    a.insert(id(2), MemoryAvailability::Unsupported);
    assert_eq!(
        run(&[e], &g, &m, &authorized, &a, 4)[0].candidates,
        out.candidates
    );
}

#[test]
fn historical_variable_roots_merge_by_max_with_isolated_roots_and_worker_equality() {
    let g = graph(&[(1, 3), (2, 3), (3, 1)]);
    let a = available(&[1, 2, 3, 4]);
    let auth = a.keys().copied().collect();
    let m = (1..=4).map(|n| (id(n), community(1))).collect();
    let e = event(
        HistoricalPropagationKind::Semantic,
        &[(1, 10), (2, 9), (4, 1)],
    );
    let events = vec![e; 32];
    let one = run(&events, &g, &m, &auth, &a, 1);
    assert_eq!(one, run(&events, &g, &m, &auth, &a, 8));
    assert_eq!(
        one[0].candidates,
        [(id(1), 10), (id(2), 9), (id(3), 5), (id(4), 1)]
            .into_iter()
            .collect()
    );
    let mut e = events[0].clone();
    e.credit_excluded.insert(id(3));
    assert!(
        !run(&[e], &g, &m, &auth, &a, 1)[0]
            .candidates
            .contains_key(&id(3))
    );
}

#[test]
fn historical_fixed_origin_reentry_unknown_community_and_authorization() {
    let g = graph(&[(1, 2), (2, 3), (3, 4), (4, 5)]);
    let a = available(&[1, 2, 3, 4, 5]);
    let auth = a.keys().copied().collect();
    let m = [
        (id(1), community(1)),
        (id(2), community(1)),
        (id(3), community(2)),
        (id(4), community(2)),
        (id(5), community(1)),
    ]
    .into_iter()
    .collect();
    let e = event(
        HistoricalPropagationKind::FirstCycleDream,
        &[(1, 50), (3, 50)],
    );
    let out = run(&[e.clone()], &g, &m, &auth, &a, 1);
    assert_eq!(out[0].candidates[&id(4)], 45);
    assert_eq!(out[0].candidates[&id(5)], 35);
    let mut single = e.clone();
    single.roots.remove(&id(3));
    assert_eq!(
        run(&[single.clone()], &g, &m, &auth, &a, 1)[0].candidates[&id(5)],
        20
    );
    assert_eq!(
        run(
            &[single.clone()],
            &g,
            &CommunityMembership::new(),
            &auth,
            &a,
            1
        )[0]
        .candidates[&id(5)],
        10
    );
    let mut restricted = auth;
    restricted.remove(&id(2));
    assert_eq!(
        run(&[single], &g, &m, &restricted, &a, 1)[0].candidates,
        [(id(1), 50)].into_iter().collect()
    );
    restricted.remove(&id(1));
    assert!(matches!(
        propagate_historical_events(
            &[e],
            &g,
            &m,
            &restricted,
            &a,
            1,
            &FreshnessPolicy::default_v1()
        ),
        Err(HistoricalPropagationError::UnauthorizedRoot(_))
    ));
}

#[test]
fn historical_validation_does_not_relax_live_principals_or_bridge_exclusions() {
    let g = graph(&[(1, 2), (2, 3)]);
    let a = available(&[1, 2, 3]);
    let auth = a.keys().copied().collect();
    let m = (1..=3).map(|n| (id(n), community(1))).collect();
    for (kind, bonus) in [
        (HistoricalPropagationKind::Semantic, 11),
        (HistoricalPropagationKind::Semantic, 0),
        (HistoricalPropagationKind::FirstCycleDream, 25),
    ] {
        assert_eq!(
            propagate_historical_events(
                &[event(kind, &[(1, bonus)])],
                &g,
                &m,
                &auth,
                &a,
                1,
                &FreshnessPolicy::default_v1()
            ),
            Err(HistoricalPropagationError::InvalidEvent)
        );
    }
    let live = PropagationEvent {
        roots: vec![id(1)],
        principal: 10,
        excluded: BTreeSet::new(),
    };
    assert_eq!(
        propagate(&live, &g, &m, 4),
        Err(PropagationError::InvalidPrincipal)
    );
    let live = PropagationEvent {
        principal: 50,
        excluded: [id(2)].into_iter().collect(),
        ..live
    };
    assert_eq!(
        propagate(&live, &g, &m, 4).unwrap(),
        [(id(1), 50)].into_iter().collect()
    );
    let policy = FreshnessPolicy {
        decay_turns_per_point: 11,
        ..FreshnessPolicy::default_v1()
    };
    assert_eq!(
        propagate_historical_events(&[], &g, &m, &auth, &a, 1, &policy),
        Err(HistoricalPropagationError::UnsupportedPolicy)
    );
}

#[test]
fn historical_semantic_handoff_keeps_one_turn_and_strongest_duplicate_root() {
    use crate::freshness::{
        historical_planner::SourceKey,
        historical_semantic::HistoricalSemanticEvent,
        semantic_policy::{SemanticEffect, SemanticPolicy},
    };
    let mut source = HistoricalSemanticEvent {
        event_id: [1; 32],
        credit_excluded: BTreeSet::from([id(3)]),
        source_key: SourceKey {
            conversation_id: "c".into(),
            node_id: "n".into(),
        },
        source_position: 10,
        owner_id: "rel-owner".into(),
        semantic_policy_digest: SemanticPolicy::default().digest().unwrap(),
        compatibility_profile_id: [1; 32],
        source_evidence_digest: [2; 32],
        target_evidence_digest: [3; 32],
        effects: vec![
            SemanticEffect {
                memory_id: id(1),
                cosine_micros: 800000,
                bonus: 3,
            },
            SemanticEffect {
                memory_id: id(1),
                cosine_micros: 1000000,
                bonus: 10,
            },
            SemanticEffect {
                memory_id: id(2),
                cosine_micros: 900000,
                bonus: 6,
            },
        ],
    };
    let e = HistoricalPropagationEvent::from_semantic(&source).unwrap();
    assert_eq!(e.roots, [(id(1), 10), (id(2), 6)].into_iter().collect());
    assert_eq!(e.position, 10);
    assert!(e.credit_excluded.contains(&id(3)));
    source.semantic_policy_digest = [0; 32];
    assert_eq!(
        HistoricalPropagationEvent::from_semantic(&source),
        Err(HistoricalPropagationError::InvalidEvent)
    );
}

#[test]
fn historical_semantic_budget_and_inclusive_recipient_position() {
    let mut e = event(HistoricalPropagationKind::Semantic, &[]);
    e.roots = (1..=64).map(|n| (id(n), 10)).collect();
    let a: BTreeMap<_, _> = (1..=65)
        .map(|n| (id(n), MemoryAvailability::Supported { position: 10 }))
        .collect();
    let auth = a.keys().copied().collect();
    assert_eq!(
        run(
            &[e.clone()],
            &MemoryAdjacency::new(),
            &CommunityMembership::new(),
            &auth,
            &a,
            1
        )[0]
        .candidates
        .len(),
        64
    );
    e.roots.insert(id(65), 10);
    assert_eq!(
        propagate_historical_events(
            &[e.clone()],
            &MemoryAdjacency::new(),
            &CommunityMembership::new(),
            &auth,
            &a,
            1,
            &FreshnessPolicy::default_v1()
        ),
        Err(HistoricalPropagationError::InvalidEvent)
    );
    e.roots.clear();
    e.position = 0;
    assert_eq!(
        propagate_historical_events(
            &[e],
            &MemoryAdjacency::new(),
            &CommunityMembership::new(),
            &auth,
            &a,
            1,
            &FreshnessPolicy::default_v1()
        ),
        Err(HistoricalPropagationError::InvalidEvent)
    );
}

#[test]
fn historical_semantic_future_root_bridges_to_eligible_recipient_without_credit() {
    let g = graph(&[(2, 3)]);
    let a: BTreeMap<MemoryId, MemoryAvailability> = [
        (id(2), MemoryAvailability::Supported { position: 11 }),
        (id(3), MemoryAvailability::Supported { position: 10 }),
    ]
    .into_iter()
    .collect();
    let authorized = a.keys().copied().collect();
    let m = [(id(2), community(1)), (id(3), community(1))]
        .into_iter()
        .collect();
    let out = run(
        &[event(HistoricalPropagationKind::Semantic, &[(2, 10)])],
        &g,
        &m,
        &authorized,
        &a,
        1,
    )
    .remove(0);
    assert_eq!(out.candidates, [(id(3), 5)].into_iter().collect());
    assert!(!out.candidates.contains_key(&id(2)));
}

#[test]
fn historical_dream_future_root_bridges_to_eligible_recipient_without_credit() {
    let g = graph(&[(2, 3)]);
    let a: BTreeMap<MemoryId, MemoryAvailability> = [
        (id(2), MemoryAvailability::Supported { position: 11 }),
        (id(3), MemoryAvailability::Supported { position: 10 }),
    ]
    .into_iter()
    .collect();
    let authorized = a.keys().copied().collect();
    let m = [(id(2), community(1)), (id(3), community(1))]
        .into_iter()
        .collect();
    let out = run(
        &[event(
            HistoricalPropagationKind::FirstCycleDream,
            &[(2, 50)],
        )],
        &g,
        &m,
        &authorized,
        &a,
        1,
    )
    .remove(0);
    assert_eq!(out.candidates, [(id(3), 45)].into_iter().collect());
    assert!(!out.candidates.contains_key(&id(2)));
}

#[test]
fn historical_semantic_credit_is_not_capped_to_the_direct_root_budget() {
    let e = event(HistoricalPropagationKind::Semantic, &[(1, 10)]);
    let edges: Vec<_> = (2..=100).map(|n| (1, n)).collect();
    let g = graph(&edges);
    let a: BTreeMap<_, _> = (1..=100)
        .map(|n| (id(n), MemoryAvailability::Supported { position: 1 }))
        .collect();
    let auth = a.keys().copied().collect();
    let m = (1..=100).map(|n| (id(n), community(1))).collect();
    let out = run(&[e], &g, &m, &auth, &a, 1).remove(0);
    assert_eq!(out.candidates.len(), 100);
    assert_eq!(out.candidates[&id(100)], 5);
}
