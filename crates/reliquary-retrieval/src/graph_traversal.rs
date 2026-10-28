use reliquary_graph::{CommunityId, GraphRelation, MemoryId};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
struct FrontierItem {
    node: MemoryId,
    depth: usize,
    boundaries: usize,
    sequence: u64,
}

/// Deterministically orders searchable Memories reachable from the supplied seeds.
///
/// Edges are treated as undirected. In community-routed mode, community boundaries
/// break ties only between nodes at the same graph depth.
pub fn traverse(
    relations: &[GraphRelation],
    memberships: &HashMap<MemoryId, CommunityId>,
    searchable: &HashSet<MemoryId>,
    seeds: &[MemoryId],
    budget: usize,
    max_depth: usize,
    community_within_depth: bool,
) -> Vec<MemoryId> {
    let adjacency = adjacency(relations, searchable);
    let mut frontier = Vec::new();
    let mut best = HashMap::<MemoryId, (usize, usize)>::new();
    let mut visited = HashSet::new();
    let mut ordered = Vec::new();
    let mut sequence = 0_u64;

    for &seed in seeds {
        if !searchable.contains(&seed) {
            continue;
        }
        best.insert(seed, (0, 0));
        frontier.push(FrontierItem {
            node: seed,
            depth: 0,
            boundaries: 0,
            sequence,
        });
        sequence += 1;
    }

    while ordered.len() < budget && !frontier.is_empty() {
        let index = frontier_index(&frontier, community_within_depth);
        let item = frontier.swap_remove(index);
        if visited.contains(&item.node)
            || best.get(&item.node) != Some(&(item.boundaries, item.depth))
        {
            continue;
        }
        visited.insert(item.node);
        ordered.push(item.node);
        if item.depth == max_depth {
            continue;
        }
        for &neighbor in adjacency.get(&item.node).into_iter().flatten() {
            if visited.contains(&neighbor) {
                continue;
            }
            let crossed = memberships.get(&item.node) != memberships.get(&neighbor);
            let candidate = (item.boundaries + usize::from(crossed), item.depth + 1);
            if best
                .get(&neighbor)
                .is_none_or(|current| candidate < *current)
            {
                best.insert(neighbor, candidate);
                frontier.push(FrontierItem {
                    node: neighbor,
                    depth: candidate.1,
                    boundaries: candidate.0,
                    sequence,
                });
                sequence += 1;
            }
        }
    }
    ordered
}

fn adjacency(
    relations: &[GraphRelation],
    searchable: &HashSet<MemoryId>,
) -> HashMap<MemoryId, Vec<MemoryId>> {
    let mut values = HashMap::<MemoryId, HashSet<MemoryId>>::new();
    for relation in relations {
        if !relation.active
            || !searchable.contains(&relation.source)
            || !searchable.contains(&relation.target)
        {
            continue;
        }
        values
            .entry(relation.source)
            .or_default()
            .insert(relation.target);
        values
            .entry(relation.target)
            .or_default()
            .insert(relation.source);
    }
    values
        .into_iter()
        .map(|(memory, neighbors)| {
            let mut neighbors: Vec<_> = neighbors.into_iter().collect();
            neighbors.sort();
            (memory, neighbors)
        })
        .collect()
}

fn frontier_index(frontier: &[FrontierItem], community_within_depth: bool) -> usize {
    frontier
        .iter()
        .enumerate()
        .min_by_key(|(_, item)| {
            if community_within_depth {
                (item.depth, item.boundaries, item.sequence)
            } else {
                (item.depth, 0, item.sequence)
            }
        })
        .map(|(index, _)| index)
        .expect("non-empty retrieval frontier")
}

#[cfg(test)]
mod tests {
    use super::traverse;
    use reliquary_graph::{
        CommunityId, GraphRelation, GraphRelationKind, GraphRelationOrigin, MemoryId,
    };
    use std::collections::{HashMap, HashSet};

    #[test]
    fn community_affinity_breaks_equal_depth_ties_but_not_depth_order() {
        let a: MemoryId = [1; 32];
        let cross: MemoryId = [2; 32];
        let local: MemoryId = [3; 32];
        let deep_local: MemoryId = [4; 32];
        let relations = [
            relation(a, cross),
            relation(a, local),
            relation(local, deep_local),
        ];
        let memberships = HashMap::from([
            (a, CommunityId([10; 32])),
            (local, CommunityId([10; 32])),
            (deep_local, CommunityId([10; 32])),
            (cross, CommunityId([20; 32])),
        ]);
        let searchable = HashSet::from([a, cross, local, deep_local]);

        let global = traverse(&relations, &memberships, &searchable, &[a], 4, 3, false);
        let routed = traverse(&relations, &memberships, &searchable, &[a], 4, 3, true);

        assert_eq!(global, vec![a, cross, local, deep_local]);
        assert_eq!(routed, vec![a, local, cross, deep_local]);
    }

    #[test]
    fn excludes_inactive_or_ineligible_edges_and_preserves_seed_order() {
        let a: MemoryId = [1; 32];
        let b: MemoryId = [2; 32];
        let excluded: MemoryId = [3; 32];
        let relations = [relation(a, b), relation_with_active(a, excluded, false)];
        let searchable = HashSet::from([a, b]);
        let memberships = HashMap::<MemoryId, CommunityId>::new();

        assert_eq!(
            traverse(&relations, &memberships, &searchable, &[b, a], 4, 2, false),
            vec![b, a]
        );
    }

    fn relation(source: MemoryId, target: MemoryId) -> GraphRelation {
        relation_with_active(source, target, true)
    }

    fn relation_with_active(source: MemoryId, target: MemoryId, active: bool) -> GraphRelation {
        GraphRelation {
            source,
            target,
            kind: GraphRelationKind::Topical,
            active,
            origin: GraphRelationOrigin::Dream,
            global_version: 1,
            graph_version: 1,
        }
    }
}
