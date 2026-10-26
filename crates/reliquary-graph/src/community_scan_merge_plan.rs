use crate::community_scan_merge::{COMMUNITY_SCAN_MERGE_FAN_IN, StructuralCommunityGraph};
use std::collections::VecDeque;

pub(crate) struct ScanPlan {
    pub(crate) leaf_nodes: Vec<Vec<usize>>,
    pub(crate) leaf_edges: Vec<Vec<(usize, usize)>>,
    boundaries: Vec<Vec<Vec<(usize, usize)>>>,
}

impl ScanPlan {
    pub(crate) fn new(graph: &StructuralCommunityGraph, shard_nodes: usize) -> Self {
        let order = structural_scan_order(graph);
        let leaf_nodes = order
            .chunks(shard_nodes)
            .map(|nodes| nodes.to_vec())
            .collect::<Vec<_>>();
        let shard_count = leaf_nodes.len();
        let mut shard_by_node = vec![0; graph.nodes.len()];
        for (shard, nodes) in leaf_nodes.iter().enumerate() {
            for node in nodes {
                shard_by_node[*node] = shard;
            }
        }

        let mut leaf_edges = vec![Vec::new(); shard_count];
        let mut level_counts = Vec::new();
        let mut width = shard_count;
        while width > 1 {
            width = width.div_ceil(COMMUNITY_SCAN_MERGE_FAN_IN);
            level_counts.push(width);
        }
        let mut boundaries = level_counts
            .iter()
            .map(|count| vec![Vec::new(); *count])
            .collect::<Vec<_>>();

        for &(source, target) in &graph.edges {
            let source_shard = shard_by_node[source];
            let target_shard = shard_by_node[target];
            if source_shard == target_shard {
                leaf_edges[source_shard].push((source, target));
                continue;
            }
            let (level, parent) = common_parent(source_shard, target_shard);
            boundaries[level - 1][parent].push((source, target));
        }
        Self {
            leaf_nodes,
            leaf_edges,
            boundaries,
        }
    }

    pub(crate) fn boundary_edges(&self, level: usize, parent: usize) -> &[(usize, usize)] {
        self.boundaries
            .get(level - 1)
            .and_then(|parents| parents.get(parent))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

fn structural_scan_order(graph: &StructuralCommunityGraph) -> Vec<usize> {
    let mut adjacency = vec![Vec::new(); graph.nodes.len()];
    for &(source, target) in &graph.edges {
        adjacency[source].push(target);
        adjacency[target].push(source);
    }

    let mut seen = vec![false; graph.nodes.len()];
    let mut order = Vec::with_capacity(graph.nodes.len());
    let mut queue = VecDeque::new();
    for seed in 0..graph.nodes.len() {
        if seen[seed] {
            continue;
        }
        seen[seed] = true;
        queue.push_back(seed);
        while let Some(node) = queue.pop_front() {
            order.push(node);
            for &neighbor in &adjacency[node] {
                if !seen[neighbor] {
                    seen[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
    }
    order
}

fn common_parent(mut left: usize, mut right: usize) -> (usize, usize) {
    let mut level = 0;
    while left != right {
        left /= COMMUNITY_SCAN_MERGE_FAN_IN;
        right /= COMMUNITY_SCAN_MERGE_FAN_IN;
        level += 1;
    }
    (level, left)
}
