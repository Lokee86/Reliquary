use crate::community_scan_merge::{
    COMMUNITY_SCAN_SHARD_NODES, ScanMergeConfig, StructuralCommunityGraph, scan_merge_structural,
};
use crate::{COMMUNITY_LEIDEN_RESOLUTION, COMMUNITY_LEIDEN_SEED, MemoryId};
use leiden_rs::{GraphDataBuilder, Leiden, LeidenConfig, QualityType};
use std::time::Instant;

const COMMUNITY_SIZE: usize = 64;
const DEGREE_FORWARD: usize = 4;

#[test]
fn synthetic_scan_merge_benchmark() {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    println!(
        "layout,nodes,edges,shards,levels,workers,full_ms,scan_merge_ms,speedup,full_q,scan_q,q_delta"
    );
    for &nodes in &[1_024, 8_192, 65_536, 262_144] {
        run_case("local", nodes, false, workers);
    }
    for &nodes in &[1_024, 8_192, 65_536, 262_144] {
        run_case("interleaved", nodes, true, workers);
    }
    run_scan_only("local", 1_048_576, false, workers);
}

fn run_case(layout: &str, node_count: usize, interleaved: bool, workers: usize) {
    let graph = synthetic_graph(node_count, interleaved);
    let start = Instant::now();
    let full_quality = full_leiden_quality(&graph);
    let full = start.elapsed();

    let start = Instant::now();
    let scan = scan_merge_structural(
        &graph,
        [42; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: COMMUNITY_SCAN_SHARD_NODES,
            workers,
        },
    )
    .unwrap();
    let scan_time = start.elapsed();
    let shards = node_count.div_ceil(COMMUNITY_SCAN_SHARD_NODES);
    let levels = reduction_levels(shards);
    let speedup = full.as_secs_f64() / scan_time.as_secs_f64();
    println!(
        "{layout},{node_count},{},{shards},{levels},{workers},{:.3},{:.3},{speedup:.3},{full_quality:.8},{:.8},{:.8}",
        graph.edges.len(),
        full.as_secs_f64() * 1_000.0,
        scan_time.as_secs_f64() * 1_000.0,
        scan.quality,
        scan.quality - full_quality,
    );
}

fn run_scan_only(layout: &str, node_count: usize, interleaved: bool, workers: usize) {
    let graph = synthetic_graph(node_count, interleaved);
    let start = Instant::now();
    let scan = scan_merge_structural(
        &graph,
        [42; 16],
        1,
        1,
        ScanMergeConfig {
            shard_nodes: COMMUNITY_SCAN_SHARD_NODES,
            workers,
        },
    )
    .unwrap();
    let elapsed = start.elapsed();
    let shards = node_count.div_ceil(COMMUNITY_SCAN_SHARD_NODES);
    println!(
        "{layout},{node_count},{},{shards},{},{workers},NA,{:.3},NA,NA,{:.8},NA",
        graph.edges.len(),
        reduction_levels(shards),
        elapsed.as_secs_f64() * 1_000.0,
        scan.quality,
    );
}

fn full_leiden_quality(graph: &StructuralCommunityGraph) -> f64 {
    if graph.edges.is_empty() {
        return 0.0;
    }
    let mut builder = GraphDataBuilder::new(graph.nodes.len());
    for &(source, target) in &graph.edges {
        builder.add_edge(source, target, 1.0).unwrap();
    }
    Leiden::new(LeidenConfig {
        resolution: COMMUNITY_LEIDEN_RESOLUTION,
        seed: Some(COMMUNITY_LEIDEN_SEED),
        quality: QualityType::Modularity,
        parallel_local_moving_threshold: None,
        parallel_aggregation_threshold: None,
        ..LeidenConfig::default()
    })
    .run(&builder.build().unwrap())
    .unwrap()
    .quality
}

fn synthetic_graph(node_count: usize, interleaved: bool) -> StructuralCommunityGraph {
    assert_eq!(node_count % COMMUNITY_SIZE, 0);
    let communities = node_count / COMMUNITY_SIZE;
    let mut edges = Vec::with_capacity(node_count * DEGREE_FORWARD + communities);
    for community in 0..communities {
        for position in 0..COMMUNITY_SIZE {
            let source = mapped_node(community, position, communities, interleaved);
            for offset in 1..=DEGREE_FORWARD {
                let target = mapped_node(
                    community,
                    (position + offset) % COMMUNITY_SIZE,
                    communities,
                    interleaved,
                );
                edges.push(ordered(source, target));
            }
        }
        let next = (community + 1) % communities;
        edges.push(ordered(
            mapped_node(community, 0, communities, interleaved),
            mapped_node(next, 0, communities, interleaved),
        ));
    }
    edges.sort_unstable();
    edges.dedup();
    StructuralCommunityGraph {
        nodes: (0..node_count).map(memory).collect(),
        edges,
    }
}

fn mapped_node(community: usize, position: usize, communities: usize, interleaved: bool) -> usize {
    if interleaved {
        position * communities + community
    } else {
        community * COMMUNITY_SIZE + position
    }
}

fn ordered(left: usize, right: usize) -> (usize, usize) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn memory(index: usize) -> MemoryId {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
    bytes
}

fn reduction_levels(mut shards: usize) -> usize {
    let mut levels = 0;
    while shards > 1 {
        shards = shards.div_ceil(2);
        levels += 1;
    }
    levels
}
