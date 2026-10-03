//! Exhaustive simple-path oracle independent of the production frontier algorithm.
use reliquary_memory::{
    CommunityId, CommunityMembership, MemoryAdjacency, MemoryId, PropagationEvent,
    freshness::{FreshnessPolicy, propagate_events_with_policy, propagate_with_policy},
};
use std::collections::{BTreeMap, BTreeSet};

fn memory(n: u8) -> MemoryId {
    MemoryId([n; 32])
}

fn oracle(
    event: &PropagationEvent,
    graph: &MemoryAdjacency,
    communities: &CommunityMembership,
    policy: &FreshnessPolicy,
) -> BTreeMap<MemoryId, i16> {
    fn walk(
        node: MemoryId,
        remaining: i16,
        origin: Option<CommunityId>,
        event: &PropagationEvent,
        graph: &MemoryAdjacency,
        communities: &CommunityMembership,
        policy: &FreshnessPolicy,
        visited: &mut BTreeSet<MemoryId>,
        output: &mut BTreeMap<MemoryId, i16>,
    ) {
        if event.excluded.contains(&node) || !visited.insert(node) {
            return;
        }
        output
            .entry(node)
            .and_modify(|best| *best = (*best).max(remaining))
            .or_insert(remaining);
        for next in graph.get(&node).into_iter().flatten() {
            let cost = if origin.is_some() && communities.get(next).copied() == origin {
                policy.local_hop_cost
            } else {
                policy.outside_hop_cost
            };
            if remaining > cost {
                walk(
                    *next,
                    remaining - cost,
                    origin,
                    event,
                    graph,
                    communities,
                    policy,
                    visited,
                    output,
                );
            }
        }
        visited.remove(&node);
    }
    let mut output = BTreeMap::new();
    for root in &event.roots {
        walk(
            *root,
            event.principal,
            communities.get(root).copied(),
            event,
            graph,
            communities,
            policy,
            &mut BTreeSet::new(),
            &mut output,
        );
    }
    output
}

#[test]
fn freshness_frontiers_match_every_simple_path_on_all_four_node_graphs() {
    let policy = FreshnessPolicy::default_v1();
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    // Every undirected graph, with cycles, disconnected vertices and re-entry.
    for mask in 0..64 {
        let mut graph = MemoryAdjacency::new();
        for (bit, (a, b)) in pairs.iter().enumerate() {
            if mask & (1 << bit) != 0 {
                graph.entry(memory(*a)).or_default().insert(memory(*b));
                graph.entry(memory(*b)).or_default().insert(memory(*a));
            }
        }
        for membership_case in 0..3 {
            let communities: CommunityMembership = (0..4)
                .filter_map(|n| {
                    if membership_case == 2 && n % 2 == 1 {
                        None
                    } else {
                        Some((
                            memory(n),
                            CommunityId([if membership_case == 0 { 7 } else { n % 2 }; 32]),
                        ))
                    }
                })
                .collect();
            for excluded in [
                BTreeSet::new(),
                BTreeSet::from([memory(1)]),
                BTreeSet::from([memory(0)]),
            ] {
                let events = [
                    PropagationEvent {
                        roots: vec![memory(0)],
                        principal: 25,
                        excluded: excluded.clone(),
                    },
                    PropagationEvent {
                        roots: vec![memory(0), memory(3), memory(0)],
                        principal: 50,
                        excluded,
                    },
                ];
                let expected = events
                    .iter()
                    .map(|event| oracle(event, &graph, &communities, &policy))
                    .collect::<Vec<_>>();
                for workers in [1, 3] {
                    for (event, expected) in events.iter().zip(&expected) {
                        assert_eq!(
                            propagate_with_policy(event, &graph, &communities, workers, &policy)
                                .unwrap(),
                            *expected,
                            "graph={mask}, membership={membership_case}, workers={workers}"
                        );
                    }
                    let batch = propagate_events_with_policy(
                        &events,
                        &graph,
                        &communities,
                        workers,
                        &policy,
                    )
                    .unwrap();
                    for (actual, expected) in batch.iter().zip(&expected) {
                        assert_eq!(actual.candidates, *expected);
                    }
                }
            }
        }
    }
}
