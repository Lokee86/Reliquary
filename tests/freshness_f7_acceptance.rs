//! Differential acceptance coverage for the public Freshness engine.
//! The oracle is structurally independent of the production max-heap traversal:
//! it enumerates every simple path in these bounded graphs before choosing a winner.
use reliquary_memory::{
    CommunityId, MemoryId,
    freshness::{
        CommunityMembership, FreshnessRecord, FreshnessState, MemoryAdjacency, PropagationEvent,
        propagate,
    },
};
use std::collections::{BTreeMap, BTreeSet};

fn memory(n: u8) -> MemoryId {
    let mut bytes = [0; 32];
    bytes[0] = n;
    MemoryId(bytes)
}

fn community(n: u8) -> CommunityId {
    let mut bytes = [0; 32];
    bytes[0] = n;
    CommunityId(bytes)
}

fn connect(graph: &mut MemoryAdjacency, a: u8, b: u8) {
    graph.entry(memory(a)).or_default().insert(memory(b));
    graph.entry(memory(b)).or_default().insert(memory(a));
}

fn reference(
    event: &PropagationEvent,
    graph: &MemoryAdjacency,
    memberships: &CommunityMembership,
) -> BTreeMap<MemoryId, i16> {
    fn visit(
        at: MemoryId,
        remaining: i16,
        origin: Option<CommunityId>,
        excluded: &BTreeSet<MemoryId>,
        graph: &MemoryAdjacency,
        memberships: &CommunityMembership,
        path: &mut BTreeSet<MemoryId>,
        best: &mut BTreeMap<MemoryId, i16>,
    ) {
        if remaining <= 0 || excluded.contains(&at) {
            return;
        }
        let entry = best.entry(at).or_insert(i16::MIN);
        if remaining > *entry {
            *entry = remaining;
        }
        path.insert(at);
        for next in graph.get(&at).into_iter().flatten() {
            if path.contains(next) || excluded.contains(next) {
                continue;
            }
            let local = origin.is_some() && memberships.get(next).copied() == origin;
            visit(
                *next,
                remaining - if local { 5 } else { 10 },
                origin,
                excluded,
                graph,
                memberships,
                path,
                best,
            );
        }
        path.remove(&at);
    }

    let mut result = BTreeMap::<MemoryId, i16>::new();
    for root in event.roots.iter().copied().collect::<BTreeSet<_>>() {
        if event.excluded.contains(&root) {
            continue;
        }
        let mut path = BTreeSet::new();
        let mut per_root = BTreeMap::new();
        visit(
            root,
            event.principal,
            memberships.get(&root).copied(),
            &event.excluded,
            graph,
            memberships,
            &mut path,
            &mut per_root,
        );
        for (id, value) in per_root {
            result
                .entry(id)
                .and_modify(|old| *old = (*old).max(value))
                .or_insert(value);
        }
    }
    result
}

#[test]
fn concurrent_propagation_matches_exhaustive_oracle_on_cycles_and_cross_community_routes() {
    let mut graph = MemoryAdjacency::new();
    for (a, b) in [
        (1, 2),
        (2, 3),
        (1, 3),
        (2, 4),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 2),
        (3, 8),
        (8, 4),
        (4, 7),
        (7, 9),
    ] {
        connect(&mut graph, a, b);
    }
    let memberships: CommunityMembership = [
        (1, 1),
        (2, 1),
        (3, 1),
        (4, 2),
        (5, 2),
        (6, 1),
        (8, 2),
        (7, 2),
        (9, 2),
    ]
    .into_iter()
    .map(|(node, group)| (memory(node), community(group)))
    .collect();
    for principal in [25, 50] {
        let event = PropagationEvent {
            roots: vec![memory(1), memory(5), memory(3), memory(1)],
            principal,
            excluded: [memory(7)].into_iter().collect(),
        };
        let expected = reference(&event, &graph, &memberships);
        assert_eq!(
            propagate(&event, &graph, &memberships, 1).unwrap(),
            expected
        );
        for workers in [2, 3, 8] {
            assert_eq!(
                propagate(&event, &graph, &memberships, workers).unwrap(),
                expected,
                "worker count {workers}, principal {principal}"
            );
        }
        assert!(!expected.contains_key(&memory(7)));
        assert!(
            !expected.contains_key(&memory(9)),
            "excluded source cannot bridge"
        );
    }
}

#[test]
fn score_boundaries_remainders_and_floor_are_observable_through_public_api() {
    let mut record = FreshnessRecord::created();
    assert!(record.admit(37).unwrap());
    assert!(!record.admit(900).unwrap());
    assert_eq!(record.score_at(1027).unwrap(), 1);
    record.reinforce(25, 1048).unwrap();
    assert_eq!(record.score, 24);
    assert_eq!(record.accounted_turn, Some(1047));
    assert_eq!(record.score_at(1056).unwrap(), 24);
    assert_eq!(record.score_at(1057).unwrap(), 23);

    // Reinforcement that saturates at +100 must still retain the activity remainder.
    let mut capped = FreshnessRecord::created();
    capped.admit(37).unwrap();
    capped.reinforce(25, 48).unwrap();
    assert_eq!(capped.score, 100);
    assert_eq!(capped.accounted_turn, Some(47));
    assert_eq!(capped.score_at(56).unwrap(), 100);
    assert_eq!(capped.score_at(57).unwrap(), 99);

    // Reaching the floor does not discard a previously accumulated decay remainder.
    // The last accounted boundary is 1057, so decay resumes at 1067 after reinforcement.
    record.score = -100;
    record.reinforce(25, 1066).unwrap();
    assert_eq!(record.score, -75);
    assert_eq!(record.accounted_turn, Some(1057));
    assert_eq!(record.state_at(1066).unwrap(), FreshnessState::Dormant);
    assert_eq!(record.score_at(1066).unwrap(), -75);
    assert_eq!(record.score_at(1067).unwrap(), -76);

    // Event propagation uses the +25 principal even when the source remains Dormant.
    let mut dormant = FreshnessRecord::created();
    dormant.admit(0).unwrap();
    dormant.score = -100;
    dormant.reinforce(25, 0).unwrap();
    assert_eq!(dormant.score, -75);
    let mut graph = MemoryAdjacency::new();
    connect(&mut graph, 20, 21);
    let memberships: CommunityMembership = [(memory(20), community(3)), (memory(21), community(3))]
        .into_iter()
        .collect();
    let event = PropagationEvent {
        roots: vec![memory(20)],
        principal: 25,
        excluded: BTreeSet::new(),
    };
    assert_eq!(
        propagate(&event, &graph, &memberships, 1).unwrap()[&memory(21)],
        20
    );
}
