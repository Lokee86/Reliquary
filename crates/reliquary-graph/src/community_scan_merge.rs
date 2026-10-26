use crate::community_leiden::structural_edges;
use crate::community_store::community_id;
use crate::graph_store::GraphStore;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_LEIDEN_RESOLUTION, COMMUNITY_LEIDEN_SEED, Community,
    CommunityError, CommunitySnapshot, MemoryId,
};
use std::collections::BTreeMap;

pub(crate) const COMMUNITY_SCAN_SHARD_NODES: usize = 2_048;
pub(crate) const COMMUNITY_SCAN_MERGE_FAN_IN: usize = 2;

#[derive(Clone, Debug)]
pub(crate) struct StructuralCommunityGraph {
    pub(crate) nodes: Vec<MemoryId>,
    pub(crate) edges: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScanMergeConfig {
    pub(crate) shard_nodes: usize,
    pub(crate) workers: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RegionSummary {
    pub(crate) groups: Vec<Vec<usize>>,
    pub(crate) edges: BTreeMap<(usize, usize), f64>,
    pub(crate) quality: f64,
}

pub fn scan_merge_snapshot(
    graph: &GraphStore,
    owner_uuid: [u8; 16],
    generation: u64,
) -> Result<CommunitySnapshot, CommunityError> {
    let structural = StructuralCommunityGraph {
        nodes: graph.node_memory_ids(),
        edges: structural_edges(graph)?.into_iter().collect(),
    };
    scan_merge_structural(
        &structural,
        owner_uuid,
        generation,
        graph.memory_graph_version(),
        default_config(),
    )
}

pub(crate) fn scan_merge_structural(
    graph: &StructuralCommunityGraph,
    owner_uuid: [u8; 16],
    generation: u64,
    graph_version: u64,
    config: ScanMergeConfig,
) -> Result<CommunitySnapshot, CommunityError> {
    if graph.nodes.is_empty() {
        return Ok(snapshot(generation, graph_version, 0.0, Vec::new()));
    }
    let summary = crate::community_scan_merge_reduce::reduce_graph(graph, config)?;
    let mut communities = summary
        .groups
        .into_iter()
        .map(|members| make_community(graph, owner_uuid, members))
        .collect::<Vec<_>>();
    communities.sort_by_key(|community| community.id);
    Ok(snapshot(
        generation,
        graph_version,
        summary.quality,
        communities,
    ))
}

pub(crate) fn default_config() -> ScanMergeConfig {
    ScanMergeConfig {
        shard_nodes: COMMUNITY_SCAN_SHARD_NODES,
        workers: std::thread::available_parallelism().map_or(1, usize::from),
    }
}

fn make_community(
    graph: &StructuralCommunityGraph,
    owner_uuid: [u8; 16],
    node_members: Vec<usize>,
) -> Community {
    let mut members = node_members
        .into_iter()
        .map(|node| graph.nodes[node])
        .collect::<Vec<_>>();
    members.sort_by_key(|member| *member);
    Community {
        id: community_id(owner_uuid, &members),
        members,
    }
}

fn snapshot(
    generation: u64,
    derived_graph_version: u64,
    quality: f64,
    communities: Vec<Community>,
) -> CommunitySnapshot {
    CommunitySnapshot {
        generation,
        derived_graph_version,
        algorithm_version: COMMUNITY_ALGORITHM_VERSION,
        seed: COMMUNITY_LEIDEN_SEED,
        resolution: COMMUNITY_LEIDEN_RESOLUTION,
        quality,
        communities,
    }
}
