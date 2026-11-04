use reliquary_graph::{CommunityId, NodeRef, Relation};
use reliquary_memory_core::MemoryId;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
struct FrontierItem {
    node: MemoryId,
    depth: usize,
    boundaries: usize,
    sequence: u64,
}

/// Deterministically orders searchable Memories reachable from supplied seeds.
/// Relations are treated as undirected; only active Memory-to-Memory edges count.
pub fn traverse(
    relations: &[Relation],
    memberships: &HashMap<MemoryId, CommunityId>,
    searchable: &HashSet<MemoryId>,
    seeds: &[MemoryId],
    budget: usize,
    max_depth: usize,
    community_within_depth: bool,
) -> Vec<MemoryId> {
    if budget == 0 || searchable.is_empty() {
        return Vec::new();
    }
    let adjacency = adjacency(relations, searchable);
    let mut frontier = Vec::new();
    let mut best = HashMap::<MemoryId, (usize, usize)>::new();
    let mut visited = HashSet::new();
    let mut ordered = Vec::new();
    let mut sequence = 0_u64;

    for &seed in seeds {
        if searchable.contains(&seed) && !best.contains_key(&seed) {
            best.insert(seed, (0, 0));
            frontier.push(FrontierItem {
                node: seed,
                depth: 0,
                boundaries: 0,
                sequence,
            });
            sequence += 1;
        }
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
                sequence = sequence.saturating_add(1);
            }
        }
    }
    ordered
}

fn adjacency(
    relations: &[Relation],
    searchable: &HashSet<MemoryId>,
) -> HashMap<MemoryId, Vec<MemoryId>> {
    let mut values = HashMap::<MemoryId, HashSet<MemoryId>>::new();
    for relation in relations {
        if !relation.active {
            continue;
        }
        let (NodeRef::Memory(source), NodeRef::Memory(target)) = (relation.source, relation.target)
        else {
            continue;
        };
        let source = MemoryId(source);
        let target = MemoryId(target);
        if !searchable.contains(&source) || !searchable.contains(&target) {
            continue;
        }
        values.entry(source).or_default().insert(target);
        values.entry(target).or_default().insert(source);
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
    use super::*;
    use reliquary_graph::{RelationKind, RelationOrigin};

    fn id(value: u8) -> MemoryId {
        MemoryId([value; 32])
    }
    fn community(value: u8) -> CommunityId {
        CommunityId([value; 32])
    }
    fn edge(source: u8, target: u8, active: bool) -> Relation {
        Relation {
            source: NodeRef::Memory([source; 32]),
            target: NodeRef::Memory([target; 32]),
            kind: RelationKind::Topical,
            active,
            origin: RelationOrigin::Dream,
            graph_version: 1,
        }
    }

    #[test]
    fn undirected_bounded_traversal_is_stable_and_ignores_inactive_edges() {
        let relations = [edge(1, 2, true), edge(1, 3, true), edge(3, 4, false)];
        let found = traverse(
            &relations,
            &HashMap::new(),
            &[1, 2, 3, 4].map(id).into(),
            &[id(1)],
            8,
            4,
            false,
        );
        assert_eq!(found, [id(1), id(2), id(3)]);
    }

    #[test]
    fn community_boundaries_break_only_equal_depth_ties() {
        let relations = [
            edge(1, 2, true),
            edge(1, 3, true),
            edge(2, 4, true),
            edge(3, 5, true),
        ];
        let membership = [
            (id(1), community(8)),
            (id(2), community(9)),
            (id(3), community(8)),
            (id(4), community(9)),
            (id(5), community(8)),
        ]
        .into_iter()
        .collect();
        let searchable = [1, 2, 3, 4, 5].map(id).into();
        let result = traverse(&relations, &membership, &searchable, &[id(1)], 5, 2, true);
        assert_eq!(result, [id(1), id(3), id(2), id(5), id(4)]);
    }
}
