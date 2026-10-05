//! Reproducible Freshness propagation calibration.
//!
//! Default mode builds a deterministic synthetic graph. Pass --fixture <REL path>
//! to inspect a frozen real REL: the source is SHA-256 hashed, copied to a disposable
//! temp directory, the copy hash is checked, and Cva::open is called only on that
//! writable copy. Required --sha256 <hex> verifies the authoritative fixture digest.
//! The source digest is checked again after all measurements.
//!
//! Fixture graph/community state supports prospective topology-only +25/+50 events.
//! Historical direct-use receipts and event-time graph/community snapshots are not
//! present, so results cannot be read as reconstructed historical behavior.
//!
//! Run synthetic: cargo run --release --example freshness_calibration
//! Run real: cargo run --release --example freshness_calibration -- --fixture "C:\\!bin\\workspace\\reliquary-fixtures\\local\\authoritative\\chatgpt-first14d\\project.prj.rel" --sha256 49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b
use reliquary_memory::{
    CommunityId, Cva, MemoryId,
    freshness::{CommunityMembership, MemoryAdjacency, PropagationEvent, propagate_with_stats},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;
use uuid::Uuid;

const COMMUNITIES: u8 = 32;
const PER_COMMUNITY: u8 = 64;
const EVENTS: usize = 64;
const RUNS: usize = 11;

fn mem(group: u8, local: u8) -> MemoryId {
    let mut id = [0; 32];
    id[0] = group;
    id[1] = local;
    MemoryId(id)
}

fn community(group: u8) -> CommunityId {
    let mut id = [0; 32];
    id[0] = group;
    CommunityId(id)
}

fn add_edge(graph: &mut MemoryAdjacency, a: MemoryId, b: MemoryId) {
    graph.entry(a).or_default().insert(b);
    graph.entry(b).or_default().insert(a);
}

fn synthetic_graph() -> (MemoryAdjacency, CommunityMembership) {
    let mut adjacency = MemoryAdjacency::new();
    let mut membership = CommunityMembership::new();
    for c in 0..COMMUNITIES {
        for n in 0..PER_COMMUNITY {
            membership.insert(mem(c, n), community(c));
            for delta in [1_u8, 2, 5, 11] {
                let next = (n + delta) % PER_COMMUNITY;
                add_edge(&mut adjacency, mem(c, n), mem(c, next));
            }
        }
    }
    for c in 0..COMMUNITIES {
        add_edge(
            &mut adjacency,
            mem(c, PER_COMMUNITY - 1),
            mem((c + 1) % COMMUNITIES, 0),
        );
    }
    (adjacency, membership)
}

fn digest(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65_536];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn percentile(values: &mut [u128], percent: usize) -> u128 {
    values.sort_unstable();
    values[((values.len() * percent).div_ceil(100))
        .saturating_sub(1)
        .min(values.len() - 1)]
}

fn run_measurement(
    label: &str,
    adjacency: &MemoryAdjacency,
    memberships: &CommunityMembership,
    root_pool: &[MemoryId],
) {
    let nodes = memberships.len().max(adjacency.len());
    let edges = adjacency.values().map(BTreeSet::len).sum::<usize>() / 2;
    println!(
        "fixture={label} projected_nodes={nodes} active_memory_edges={edges} community_memberships={}",
        memberships.len()
    );
    if root_pool.is_empty() {
        println!("fixture={label} no_connected_memory_roots; skipping propagation");
        return;
    }
    let excluded = BTreeSet::new();
    let mut baseline_p50 = None;
    for workers in [1_usize, 2, 4, 8] {
        let mut batch_ns = Vec::new();
        let mut event_ns = Vec::new();
        let mut touched_nodes = Vec::new();
        let mut examined_edges = Vec::new();
        let mut max_frontiers = Vec::new();
        for repetition in 0..RUNS {
            let batch_start = Instant::now();
            for event_index in 0..EVENTS {
                let offset = event_index * 13 + repetition * 31;
                let roots = (0..8)
                    .map(|i| root_pool[(offset + i * 97) % root_pool.len()])
                    .collect();
                let event = PropagationEvent {
                    roots,
                    principal: if event_index % 2 == 0 { 25 } else { 50 },
                    excluded: excluded.clone(),
                };
                let start = Instant::now();
                let outcome =
                    propagate_with_stats(&event, adjacency, memberships, workers).unwrap();
                event_ns.push(start.elapsed().as_nanos());
                touched_nodes.push(outcome.stats.touched_memories as u128);
                examined_edges.push(u128::from(outcome.stats.examined_edges));
                max_frontiers.push(outcome.stats.max_frontier_width as u128);
            }
            batch_ns.push(batch_start.elapsed().as_nanos());
        }
        let mut values = batch_ns.clone();
        let batch_p50 = percentile(&mut values, 50);
        let mut values = batch_ns.clone();
        let batch_p95 = percentile(&mut values, 95);
        let mut values = batch_ns.clone();
        let batch_p99 = percentile(&mut values, 99);
        let mut values = event_ns.clone();
        let event_p50 = percentile(&mut values, 50);
        let mut values = event_ns.clone();
        let event_p95 = percentile(&mut values, 95);
        let mut values = event_ns.clone();
        let event_p99 = percentile(&mut values, 99);
        let mut values = touched_nodes.clone();
        let node_p50 = percentile(&mut values, 50);
        let mut values = touched_nodes.clone();
        let node_p95 = percentile(&mut values, 95);
        let mut values = touched_nodes.clone();
        let node_p99 = percentile(&mut values, 99);
        let mut values = examined_edges.clone();
        let edge_p50 = percentile(&mut values, 50);
        let mut values = examined_edges.clone();
        let edge_p95 = percentile(&mut values, 95);
        let mut values = examined_edges.clone();
        let edge_p99 = percentile(&mut values, 99);
        let mut values = max_frontiers.clone();
        let frontier_p50 = percentile(&mut values, 50);
        let mut values = max_frontiers.clone();
        let frontier_p95 = percentile(&mut values, 95);
        let mut values = max_frontiers.clone();
        let frontier_p99 = percentile(&mut values, 99);
        if workers == 1 {
            baseline_p50 = Some(batch_p50);
        }
        let speedup = baseline_p50
            .map(|single| single as f64 / batch_p50 as f64)
            .unwrap_or(1.0);
        println!(
            "workers={workers} batch_ms_p50={:.3} p95={:.3} p99={:.3} event_us_p50={:.3} p95={:.3} p99={:.3} touched_p50={node_p50} p95={node_p95} p99={node_p99} examined_edges_p50={edge_p50} p95={edge_p95} p99={edge_p99} max_frontier_p50={frontier_p50} p95={frontier_p95} p99={frontier_p99} batch_speedup_vs_1={speedup:.3}",
            batch_p50 as f64 / 1_000_000.0,
            batch_p95 as f64 / 1_000_000.0,
            batch_p99 as f64 / 1_000_000.0,
            event_p50 as f64 / 1_000.0,
            event_p95 as f64 / 1_000.0,
            event_p99 as f64 / 1_000.0,
        );
    }
}

fn real_fixture(path: &Path, expected: Option<&str>) -> Result<(), Box<dyn Error>> {
    let expected =
        expected.ok_or("real fixtures require --sha256 with the authoritative digest")?;
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("--sha256 must contain exactly 64 hexadecimal digits".into());
    }
    let original_hash = digest(path)?;
    if !original_hash.eq_ignore_ascii_case(expected) {
        return Err(
            format!("fixture SHA-256 mismatch: got {original_hash}, expected {expected}").into(),
        );
    }
    let scratch_dir =
        std::env::temp_dir().join(format!("freshness-calibration-{}", Uuid::new_v4()));
    fs::create_dir_all(&scratch_dir)?;
    let scratch = Scratch(scratch_dir.clone());
    let copy_path = scratch_dir.join("fixture.rel");
    fs::copy(path, &copy_path)?;
    let mut permissions = fs::metadata(&copy_path)?.permissions();
    permissions.set_readonly(false);
    fs::set_permissions(&copy_path, permissions)?;
    if digest(&copy_path)? != original_hash {
        return Err("disposable fixture copy hash differs from source".into());
    }

    let open_start = Instant::now();
    println!("fixture_copy_hash_verified=true opening_owner=true");
    let cva = Cva::open(&copy_path)?;
    println!("owner_open_ms={}", open_start.elapsed().as_millis());
    let pin_start = Instant::now();
    let snapshot = cva.community_snapshot();
    let community_is_current = snapshot.as_ref().is_some_and(|snapshot| {
        cva.community_stats().current
            && snapshot.derived_graph_version == cva.memory_graph_version()
    });
    let mut adjacency = MemoryAdjacency::new();
    for relation in cva
        .graph_relations()
        .into_iter()
        .filter(|relation| relation.active)
    {
        add_edge(&mut adjacency, relation.source, relation.target);
    }
    let mut memberships = CommunityMembership::new();
    if community_is_current {
        for community in &snapshot.unwrap().communities {
            for memory in &community.members {
                memberships.insert(*memory, community.id);
            }
        }
    }
    let mut roots: Vec<_> = adjacency.keys().copied().collect();
    roots.sort_unstable();
    let label = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("real_rel");
    println!(
        "real_fixture={label} sha256={original_hash} copy_hash_verified=true community_snapshot_current={community_is_current} rel_turns={}",
        cva.rel_turn_count()
    );
    println!(
        "graph_pinning_ms={} graph_density={:.8}",
        pin_start.elapsed().as_millis(),
        if roots.len() > 1 {
            adjacency.values().map(BTreeSet::len).sum::<usize>() as f64
                / (roots.len() * (roots.len() - 1)) as f64
        } else {
            0.0
        }
    );
    run_measurement(label, &adjacency, &memberships, &roots);
    drop(cva);
    if digest(path)? != original_hash {
        return Err("source fixture changed during disposable-copy inspection".into());
    }
    println!("source_hash_unchanged=true");
    drop(scratch);
    Ok(())
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut fixture = None;
    let mut expected_sha256 = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fixture" => fixture = args.next(),
            "--sha256" => expected_sha256 = args.next(),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if let Some(path) = fixture {
        return real_fixture(Path::new(&path), expected_sha256.as_deref());
    }
    if expected_sha256.is_some() {
        return Err("--sha256 requires --fixture".into());
    }
    let (adjacency, memberships) = synthetic_graph();
    let roots = memberships.keys().copied().collect::<Vec<_>>();
    run_measurement("synthetic_dense_local_v1", &adjacency, &memberships, &roots);
    Ok(())
}
