//! Exhaustive simple-path oracle independent of the production frontier algorithm.
use reliquary_memory::{
    CommunityId, CommunityMembership, MemoryAdjacency, MemoryId, PropagationEvent,
    freshness::{FreshnessPolicy, propagate_events_with_policy, propagate_with_policy},
};
use std::collections::{BTreeMap, BTreeSet};

fn memory(n: u8) -> MemoryId {
    MemoryId([n; 32])
}

fn community(n: u8) -> CommunityId {
    CommunityId([n; 32])
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

/// Enumerate each partial set partition once: assigned nodes use canonical community
/// labels 0..k, and every unassigned node has no Community.
fn canonical_memberships() -> Vec<CommunityMembership> {
    fn assign(
        node: u8,
        next_label: u8,
        labels: &mut [Option<u8>; 4],
        output: &mut Vec<CommunityMembership>,
    ) {
        if node == 4 {
            output.push(
                labels
                    .iter()
                    .enumerate()
                    .filter_map(|(index, label)| {
                        label.map(|label| (memory(index as u8), community(label)))
                    })
                    .collect(),
            );
            return;
        }
        labels[node as usize] = None;
        assign(node + 1, next_label, labels, output);
        for label in 0..=next_label {
            labels[node as usize] = Some(label);
            assign(
                node + 1,
                next_label + u8::from(label == next_label),
                labels,
                output,
            );
        }
        labels[node as usize] = None;
    }

    let mut output = Vec::new();
    assign(0, 0, &mut [None; 4], &mut output);
    output
}

fn graph_for(mask: u8) -> MemoryAdjacency {
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let mut graph = MemoryAdjacency::new();
    for (bit, (a, b)) in pairs.iter().enumerate() {
        if mask & (1 << bit) != 0 {
            graph.entry(memory(*a)).or_default().insert(memory(*b));
            graph.entry(memory(*b)).or_default().insert(memory(*a));
        }
    }
    graph
}

fn subset(mask: u8) -> BTreeSet<MemoryId> {
    (0..4)
        .filter(|node| mask & (1 << node) != 0)
        .map(memory)
        .collect()
}

#[test]
fn simple_path_oracle_covers_all_four_node_graphs_and_partial_community_partitions() {
    let memberships = canonical_memberships();
    assert_eq!(
        memberships.len(),
        52,
        "all partial set partitions are canonical"
    );

    // Exhaustively cover each graph, partial partition, root subset (including empty),
    // and exclusion subset. Both default principals run for every exact tuple. A
    // representative one-in-17 slice exercises both principals under a coherent
    // nondefault policy without multiplying the entire exhaustive product again.
    let default_policy = FreshnessPolicy::default_v1();
    let nondefault_policy = FreshnessPolicy {
        decay_turns_per_point: 5,
        access_principal: 35,
        linkage_principal: 70,
        local_hop_cost: 7,
        outside_hop_cost: 14,
    };
    assert!(nondefault_policy.validate().is_ok());
    let mut case = 0usize;
    for graph_mask in 0..64 {
        let graph = graph_for(graph_mask);
        for (partition, communities) in memberships.iter().enumerate() {
            for roots_mask in 0..16 {
                for excluded_mask in 0..16 {
                    let roots = (0..4)
                        .filter(|node| roots_mask & (1 << node) != 0)
                        .map(memory)
                        .collect::<Vec<_>>();
                    let excluded = subset(excluded_mask);
                    for principal in [25, 50] {
                        let event = PropagationEvent {
                            roots: roots.clone(),
                            principal,
                            excluded: excluded.clone(),
                        };
                        let expected = oracle(&event, &graph, communities, &default_policy);
                        assert_eq!(
                            propagate_with_policy(&event, &graph, communities, 1, &default_policy,)
                                .unwrap(),
                            expected,
                            "graph={graph_mask}, partition={partition}, roots={roots_mask:#06b}, excluded={excluded_mask:#06b}, default principal={principal}"
                        );
                    }
                    if case % 17 == 0 {
                        for principal in [35, 70] {
                            let event = PropagationEvent {
                                roots: roots.clone(),
                                principal,
                                excluded: excluded.clone(),
                            };
                            let expected = oracle(&event, &graph, communities, &nondefault_policy);
                            assert_eq!(
                                propagate_with_policy(
                                    &event,
                                    &graph,
                                    communities,
                                    1,
                                    &nondefault_policy,
                                )
                                .unwrap(),
                                expected,
                                "graph={graph_mask}, partition={partition}, roots={roots_mask:#06b}, excluded={excluded_mask:#06b}, nondefault principal={principal}"
                            );
                        }
                    }
                    case += 1;
                }
            }
        }
    }
}

#[test]
fn parallel_workers_and_batch_order_match_independent_oracle() {
    let policy = FreshnessPolicy::default_v1();
    let graph = graph_for(0b11_1011);
    let communities = [
        (memory(0), community(0)),
        (memory(1), community(0)),
        (memory(2), community(1)),
        (memory(3), community(0)),
    ]
    .into_iter()
    .collect::<CommunityMembership>();
    let events = vec![
        PropagationEvent {
            roots: vec![memory(3), memory(0), memory(3)],
            principal: 50,
            excluded: BTreeSet::new(),
        },
        PropagationEvent {
            roots: vec![memory(2), memory(0)],
            principal: 25,
            excluded: [memory(1)].into_iter().collect(),
        },
        PropagationEvent {
            roots: vec![memory(1)],
            principal: 50,
            excluded: [memory(0)].into_iter().collect(),
        },
        PropagationEvent {
            roots: vec![memory(0), memory(2), memory(0)],
            principal: 25,
            excluded: BTreeSet::new(),
        },
    ];
    let expected = events
        .iter()
        .map(|event| oracle(event, &graph, &communities, &policy))
        .collect::<Vec<_>>();
    for workers in [1, 2, 4, 8] {
        let actual =
            propagate_events_with_policy(&events, &graph, &communities, workers, &policy).unwrap();
        assert_eq!(
            actual
                .iter()
                .map(|outcome| &outcome.candidates)
                .collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>(),
            "workers={workers}"
        );
        // Batch results are positional: reversing input reverses output, without changing
        // the independently computed outcome for each event.
        let reversed = events.iter().cloned().rev().collect::<Vec<_>>();
        let reversed_actual =
            propagate_events_with_policy(&reversed, &graph, &communities, workers, &policy)
                .unwrap();
        assert_eq!(
            reversed_actual
                .iter()
                .map(|outcome| &outcome.candidates)
                .collect::<Vec<_>>(),
            expected.iter().rev().collect::<Vec<_>>(),
            "reversed batch workers={workers}"
        );
    }
}

#[test]
fn competing_routes_merge_to_the_maximum_candidate() {
    // Root 0 reaches node 2 through a weaker outside-Community route, while root 3
    // reaches it directly within Community 7. The event result must retain the maximum
    // across independent root-origin searches, regardless of duplicate/unsorted roots.
    let mut graph = graph_for(0);
    for (a, b) in [(0, 1), (1, 2), (0, 3), (3, 2)] {
        graph.entry(memory(a)).or_default().insert(memory(b));
        graph.entry(memory(b)).or_default().insert(memory(a));
    }
    let communities = [
        (memory(0), community(7)),
        (memory(1), community(8)),
        (memory(2), community(7)),
        (memory(3), community(7)),
    ]
    .into_iter()
    .collect::<CommunityMembership>();
    let event = PropagationEvent {
        roots: vec![memory(0), memory(0), memory(3)],
        principal: 50,
        excluded: BTreeSet::new(),
    };
    let expected = oracle(&event, &graph, &communities, &FreshnessPolicy::default_v1());
    assert_eq!(expected.get(&memory(2)), Some(&45));
    for workers in [1, 2, 4, 8] {
        assert_eq!(
            propagate_with_policy(
                &event,
                &graph,
                &communities,
                workers,
                &FreshnessPolicy::default_v1(),
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn fixed_seed_larger_graphs_match_simple_path_oracle() {
    // Small fixed-seed corpus stays bounded while exploring graph shapes beyond four
    // nodes. Restrict nodes to six so the independent simple-path walk remains cheap.
    let policy = FreshnessPolicy {
        decay_turns_per_point: 5,
        access_principal: 35,
        linkage_principal: 70,
        local_hop_cost: 7,
        outside_hop_cost: 14,
    };
    let mut seed = 0x4d59_5df4_d0f3_3173u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for case in 0..40 {
        let count = 5 + (next() as usize % 2);
        let mut graph = MemoryAdjacency::new();
        for a in 0..count {
            for b in (a + 1)..count {
                // Every fourth graph has a hard split, guaranteeing disconnected
                // components while the remaining cases retain unrestricted random edges.
                if case % 4 == 0 && (a < count / 2) != (b < count / 2) {
                    continue;
                }
                if next() % 3 != 0 {
                    graph
                        .entry(memory(a as u8))
                        .or_default()
                        .insert(memory(b as u8));
                    graph
                        .entry(memory(b as u8))
                        .or_default()
                        .insert(memory(a as u8));
                }
            }
        }
        let communities = (0..count)
            .filter_map(|node| {
                let value = next() % 4;
                (value != 0).then(|| (memory(node as u8), community((value - 1) as u8)))
            })
            .collect::<CommunityMembership>();
        let roots = (0..count)
            .filter(|_| next() % 2 == 0)
            .map(|node| memory(node as u8))
            .collect::<Vec<_>>();
        let excluded = (0..count)
            .filter(|_| next() % 5 == 0)
            .map(|node| memory(node as u8))
            .collect();
        let event = PropagationEvent {
            roots,
            principal: if case % 2 == 0 { 35 } else { 70 },
            excluded,
        };
        let expected = oracle(&event, &graph, &communities, &policy);
        assert_eq!(
            propagate_with_policy(&event, &graph, &communities, 1, &policy).unwrap(),
            expected,
            "fixed-seed random case {case}"
        );
    }
}
