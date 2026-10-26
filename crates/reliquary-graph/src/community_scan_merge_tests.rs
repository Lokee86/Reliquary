use crate::community_scan_merge::{
    ScanMergeConfig, StructuralCommunityGraph, scan_merge_structural,
};
use crate::{COMMUNITY_LEIDEN_RESOLUTION, CommunitySnapshot, MemoryId};
use std::collections::{HashMap, HashSet};

fn memory(index: usize) -> MemoryId {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
    bytes
}

fn graph(node_count: usize, edges: &[(usize, usize)]) -> StructuralCommunityGraph {
    StructuralCommunityGraph {
        nodes: (0..node_count).map(memory).collect(),
        edges: edges.to_vec(),
    }
}

fn member_sets(snapshot: &CommunitySnapshot) -> HashSet<Vec<[u8; 32]>> {
    snapshot
        .communities
        .iter()
        .map(|community| community.members.iter().map(|member| *member).collect())
        .collect()
}

#[test]
fn scan_merge_preserves_two_dense_regions_across_shards() {
    let input = graph(6, &[(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5), (2, 3)]);
    let snapshot = scan_merge_structural(
        &input,
        [7; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: 3,
            workers: 2,
        },
    )
    .unwrap();
    let expected = [
        vec![memory(0), memory(1), memory(2)],
        vec![memory(3), memory(4), memory(5)],
    ]
    .into_iter()
    .collect();
    assert_eq!(member_sets(&snapshot), expected);
}

#[test]
fn scan_merge_can_join_one_community_split_across_shards() {
    let mut edges = Vec::new();
    for left in 0..8 {
        for right in (left + 1)..8 {
            edges.push((left, right));
        }
    }
    let snapshot = scan_merge_structural(
        &graph(8, &edges),
        [8; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: 4,
            workers: 2,
        },
    )
    .unwrap();
    assert_eq!(snapshot.communities.len(), 1);
    assert_eq!(snapshot.communities[0].members.len(), 8);
}

#[test]
fn scan_merge_is_worker_count_deterministic() {
    let mut edges = Vec::new();
    for community in 0..8 {
        let start = community * 8;
        for node in start..start + 8 {
            for offset in 1..=2 {
                let target = start + (node - start + offset) % 8;
                let pair = if node < target {
                    (node, target)
                } else {
                    (target, node)
                };
                edges.push(pair);
            }
        }
        if community + 1 < 8 {
            edges.push((start, start + 8));
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let input = graph(64, &edges);
    let serial = scan_merge_structural(
        &input,
        [9; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: 8,
            workers: 1,
        },
    )
    .unwrap();
    let parallel = scan_merge_structural(
        &input,
        [9; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: 8,
            workers: 4,
        },
    )
    .unwrap();
    assert_eq!(serial, parallel);
}

#[test]
fn scan_merge_quality_matches_final_partition_on_original_graph() {
    let mut edges = Vec::new();
    for community in 0..12 {
        let start = community * 10;
        for node in start..start + 10 {
            for offset in 1..=3 {
                let target = start + (node - start + offset) % 10;
                edges.push(if node < target {
                    (node, target)
                } else {
                    (target, node)
                });
            }
        }
        if community + 1 < 12 {
            edges.push((start + 9, start + 10));
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let input = graph(120, &edges);
    let snapshot = scan_merge_structural(
        &input,
        [10; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: 17,
            workers: 4,
        },
    )
    .unwrap();
    let quality = original_modularity(&input, &snapshot);
    assert!((quality - snapshot.quality).abs() < 1e-10);
}

fn original_modularity(graph: &StructuralCommunityGraph, snapshot: &CommunitySnapshot) -> f64 {
    if graph.edges.is_empty() {
        return 0.0;
    }
    let node_by_memory = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(node, memory)| (*memory, node))
        .collect::<HashMap<_, _>>();
    let mut community_by_node = vec![usize::MAX; graph.nodes.len()];
    for (community, value) in snapshot.communities.iter().enumerate() {
        for member in &value.members {
            community_by_node[node_by_memory[member]] = community;
        }
    }
    let mut internal = vec![0.0; snapshot.communities.len()];
    let mut degree = vec![0.0; snapshot.communities.len()];
    for &(source, target) in &graph.edges {
        let source_community = community_by_node[source];
        let target_community = community_by_node[target];
        degree[source_community] += 1.0;
        degree[target_community] += 1.0;
        if source_community == target_community {
            internal[source_community] += 1.0;
        }
    }
    let m = graph.edges.len() as f64;
    let two_m = 2.0 * m;
    (0..snapshot.communities.len())
        .map(|community| {
            internal[community] / m
                - COMMUNITY_LEIDEN_RESOLUTION * (degree[community] / two_m).powi(2)
        })
        .sum()
}
