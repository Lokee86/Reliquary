//! Deterministic Freshness propagation workload baseline.
//!
//! Synthetic workloads separate accepted-use (+25, one independent event per
//! consumed Memory) from initial-Dream (+50, multiple pre-existing roots).
//! Optional REL input is SHA-256 checked, copied to a disposable directory,
//! reopened read-write there, and checked again after measurement.
//!
//! Output is JSON with raw nanosecond samples, percentiles, per-event identities,
//! candidate-map digests and propagation statistics. Both timed paths collect stats:
//! accepted-use through the event batch API and Dream through the single-event API.
//! Peak resident bytes,
//! production event-size distributions and historical traces are explicitly
//! unavailable; this is not an end-to-end storage/runtime benchmark.
use reliquary_memory::{
    CommunityId, Cva, MemoryId,
    freshness::{
        CommunityMembership, FreshnessPolicy, MemoryAdjacency, PropagationEvent,
        propagate_events_with_policy, propagate_with_stats,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};
use uuid::Uuid;

const GROUPS: u8 = 32;
const EACH: u8 = 64;
const DEFAULT_REPETITIONS: usize = 11;
const MAX_REPETITIONS: usize = 100;
fn memory(group: u8, local: u8) -> MemoryId {
    let mut id = [0u8; 32];
    id[0] = group;
    id[1] = local;
    MemoryId(id)
}
fn community(group: u8) -> CommunityId {
    let mut id = [0u8; 32];
    id[0] = group;
    CommunityId(id)
}
fn id_hex(id: MemoryId) -> String {
    id.0.iter().map(|b| format!("{b:02x}")).collect()
}
fn add_edge(g: &mut MemoryAdjacency, a: MemoryId, b: MemoryId) {
    g.entry(a).or_default().insert(b);
    g.entry(b).or_default().insert(a);
}
fn synthetic() -> (MemoryAdjacency, CommunityMembership) {
    let (mut g, mut m) = (MemoryAdjacency::new(), CommunityMembership::new());
    for c in 0..GROUPS {
        for n in 0..EACH {
            m.insert(memory(c, n), community(c));
            for d in [1u8, 2, 5, 11] {
                add_edge(&mut g, memory(c, n), memory(c, (n + d) % EACH));
            }
        }
    }
    for c in 0..GROUPS {
        add_edge(&mut g, memory(c, EACH - 1), memory((c + 1) % GROUPS, 0));
    }
    (g, m)
}
fn percentile(v: &[u128], p: usize) -> Option<u128> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    Some(
        s[((s.len() * p).div_ceil(100))
            .saturating_sub(1)
            .min(s.len() - 1)],
    )
}
fn file_digest(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn event_digest(c: &BTreeMap<MemoryId, i16>) -> String {
    let mut h = Sha256::new();
    for (id, v) in c {
        h.update(id.0);
        h.update(v.to_le_bytes());
    }
    format!("{:x}", h.finalize())
}
fn projection_digest(g: &MemoryAdjacency, m: &CommunityMembership) -> String {
    let mut h = Sha256::new();
    h.update(b"freshness-performance-projection-v1");
    for (source, neighbors) in g {
        h.update(source.0);
        h.update((neighbors.len() as u64).to_le_bytes());
        for target in neighbors {
            h.update(target.0);
        }
    }
    h.update(b"community-membership-v1");
    for (memory, community) in m {
        h.update(memory.0);
        h.update(community.0);
    }
    format!("{:x}", h.finalize())
}
fn edge_count(g: &MemoryAdjacency) -> usize {
    g.values().map(BTreeSet::len).sum::<usize>() / 2
}

#[derive(Clone)]
struct Scenario {
    id: String,
    family: String,
    events: Vec<PropagationEvent>,
    topology: String,
}
fn event(roots: Vec<MemoryId>, principal: i16, excluded: BTreeSet<MemoryId>) -> PropagationEvent {
    PropagationEvent {
        roots,
        principal,
        excluded,
    }
}
fn scenarios(g: &MemoryAdjacency, m: &CommunityMembership) -> Vec<Scenario> {
    let mut out = Vec::new();
    let mut nodes = g.keys().copied().collect::<BTreeSet<_>>();
    for ns in g.values() {
        nodes.extend(ns.iter().copied());
    }
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    let mut communities: BTreeMap<CommunityId, Vec<MemoryId>> = BTreeMap::new();
    for (id, c) in m {
        communities.entry(*c).or_default().push(*id);
    }
    let community_lists = communities.into_values().collect::<Vec<_>>();
    for n in [1usize, 4, 16, 64] {
        if nodes.len() < n {
            continue;
        }
        let es = (0..n)
            .map(|i| event(vec![nodes[i]], 25, BTreeSet::new()))
            .collect();
        out.push(Scenario {
            id: format!("accepted_batch_{n}"),
            family: "accepted_use".into(),
            events: es,
            topology: "one_independent_single_root_plus_25_per_consumed_memory".into(),
        });
    }
    for n in [1usize, 2, 4, 8] {
        let same = community_lists
            .iter()
            .find(|x| x.len() >= n)
            .map(|x| x[..n].to_vec());
        if let Some(roots) = same {
            out.push(Scenario {
                id: format!("dream_roots_{n}_same"),
                family: "initial_dream".into(),
                events: vec![event(roots, 50, BTreeSet::new())],
                topology: "same_community".into(),
            });
        }
        if community_lists.len() >= n {
            let roots = community_lists.iter().take(n).map(|x| x[0]).collect();
            out.push(Scenario {
                id: format!("dream_roots_{n}_across"),
                family: "initial_dream".into(),
                events: vec![event(roots, 50, BTreeSet::new())],
                topology: "across_communities".into(),
            });
        }
        if nodes.len() >= n {
            let roots = nodes[..n].to_vec();
            out.push(Scenario {
                id: format!("dream_roots_{n}_unknown"),
                family: "initial_dream".into(),
                events: vec![event(roots, 50, BTreeSet::new())],
                topology: "all_root_membership_unknown".into(),
            });
        }
    }
    if let Some((&root, neighbors)) = g.iter().find(|(_, ns)| !ns.is_empty()) {
        let bridge = *neighbors.iter().next().unwrap();
        if let Some(second) = nodes
            .iter()
            .copied()
            .find(|id| *id != root && *id != bridge)
        {
            out.push(Scenario {
                id: "dream_excluded_roots_and_bridge".into(),
                family: "initial_dream".into(),
                events: vec![event(
                    vec![root, bridge, second],
                    50,
                    BTreeSet::from([root, bridge]),
                )],
                topology: "excluded_root_and_bridge_with_surviving_root".into(),
            });
        }
    }
    out
}
fn run(
    g: &MemoryAdjacency,
    m: &CommunityMembership,
    s: &Scenario,
    reps: usize,
    workers: usize,
) -> Result<Value, Box<dyn Error>> {
    let mut operation_ns = Vec::new();
    let mut records = Vec::new();
    let mut reference = Vec::new();
    let mut effective_workers = Vec::new();
    for rep in 0..reps {
        let mut rows = Vec::new();
        let mut batch_results = Vec::new();
        if s.family == "accepted_use" {
            let t = Instant::now();
            batch_results = propagate_events_with_policy(
                &s.events,
                g,
                m,
                workers,
                &FreshnessPolicy::default(),
            )?;
            operation_ns.push(t.elapsed().as_nanos());
        }
        for (i, e) in s.events.iter().enumerate() {
            let measured = if s.family == "initial_dream" {
                let t = Instant::now();
                let out = propagate_with_stats(e, g, m, workers)?;
                operation_ns.push(t.elapsed().as_nanos());
                out
            } else {
                propagate_with_stats(e, g, m, 1)?
            };
            let oracle = if s.family == "accepted_use" {
                &batch_results[i].candidates
            } else {
                &measured.candidates
            };
            let t = Instant::now();
            let serial = propagate_with_stats(e, g, m, 1)?;
            let oracle_ns = t.elapsed().as_nanos();
            if serial.candidates != *oracle {
                return Err("parallel result differs from serial reference candidate map".into());
            }
            // The public event stats describe each serial traversal, not the adaptive
            // batch pool. Do not present a requested cap as observed concurrency.
            let event_workers = if s.family == "accepted_use" && workers > 1 && s.events.len() > 1 {
                None
            } else {
                Some(measured.stats.worker_count)
            };
            effective_workers.push(event_workers);
            let origins = e
                .roots
                .iter()
                .filter(|r| !e.excluded.contains(r))
                .map(|r| {
                    m.get(r)
                        .map(|c| c.0.iter().map(|b| format!("{b:02x}")).collect::<String>())
                        .unwrap_or_else(|| "unknown".into())
                })
                .collect::<BTreeSet<_>>();
            let digest = event_digest(oracle);
            let identity = format!("{}:event{}", s.id, i);
            if rep == 0 {
                reference.push(json!({"event_identity":identity,"roots":e.roots.iter().map(|r|id_hex(*r)).collect::<Vec<_>>(),
    "excluded":e.excluded.iter().map(|r|id_hex(*r)).collect::<Vec<_>>(),"principal":e.principal,"unique_origin_groups":origins.len(),
    "candidate_count":oracle.len(),"candidate_map_sha256":digest}));
            } else if reference[i]["candidate_map_sha256"].as_str() != Some(digest.as_str()) {
                return Err("candidate map digest changed across repetitions".into());
            }
            rows.push(json!({"event_identity":identity,"candidate_count":oracle.len(),"candidate_map_sha256":digest,
    "serial_reference_ns":oracle_ns,"stats":{"candidate_expansions":measured.stats.candidate_expansions,
    "examined_edges":measured.stats.examined_edges,"touched_memories":measured.stats.touched_memories,
    "max_frontier_width":measured.stats.max_frontier_width,"frontier_levels":measured.stats.frontier_levels,
    "batch_effective_workers":event_workers,"actual_propagation_workers":measured.stats.worker_count}}));
        }
        records.push(json!({"repetition":rep,"operation_ns":if s.family=="accepted_use"{operation_ns[rep]}else{operation_ns[operation_ns.len()-s.events.len()..].iter().sum()},"events":rows}));
    }
    Ok(
        json!({"scenario_id":s.id,"family":s.family,"topology":s.topology,"requested_workers":workers,
  "effective_workers":if effective_workers.iter().any(Option::is_none){None}else{effective_workers.iter().copied().flatten().max()},"event_count":s.events.len(),"repetitions":reps,
  "worker_reporting":"actual_propagation_workers is per-event; batch concurrency unavailable from per-event public stats; requested_workers is a cap",
  "operation_latency_ns":{"raw":operation_ns,"p50":percentile(&operation_ns,50),"p95":percentile(&operation_ns,95),"p99":percentile(&operation_ns,99)},
  "events_per_second_at_p50":percentile(&operation_ns,50).map(|ns|s.events.len() as f64*1_000_000_000.0/ns as f64),
  "correctness_reference_events":reference,"samples":records}),
    )
}
fn run_all(
    label: &str,
    g: &MemoryAdjacency,
    m: &CommunityMembership,
    filter: &BTreeSet<String>,
    reps: usize,
    graph_version: Option<u64>,
    community_generation: Option<u64>,
    community_graph_version: Option<u64>,
) -> Result<Value, Box<dyn Error>> {
    let mut results = Vec::new();
    for s in scenarios(g, m)
        .iter()
        .filter(|s| filter.is_empty() || filter.contains(&s.id))
    {
        let mut sm = m.clone();
        if s.topology == "all_root_membership_unknown" {
            for e in &s.events {
                for r in &e.roots {
                    sm.remove(r);
                }
            }
        }
        for w in [1usize, 2, 4, 8] {
            results.push(run(g, &sm, s, reps, w)?);
        }
    }
    Ok(
        json!({"fixture":label,"graph":{"node_count":g.len().max(m.len()),"undirected_active_edges":edge_count(g),
  "community_membership_count":m.len(),"snapshot_validity":"see fixture metadata","active_relation_kind_breakdown":null,
  "projection_sha256":projection_digest(g,m),"graph_version":graph_version,"community_generation":community_generation,
  "community_graph_version":community_graph_version},
  "unavailable_metrics":{"peak_resident_bytes":null,"temporary_allocation_bytes":null,"production_batch_distribution":null,
  "production_dream_root_distribution":null,"exact_historical_graph_snapshot":null,"runtime_mutex_wait":null,"receipt_lookup":null,
  "durable_append_sync":null,"due_refresh":null,"reopen":null},
  "interpretation":"Propagation and independent-event batch diagnostic; does not claim full accepted-use or Dream settlement latency.",
  "unavailable_scenarios":if m.is_empty(){json!(["same/across-Community Dream scenarios require a current Community snapshot"])}else{json!([])},"results":results}),
    )
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let (mut fixture, mut sha, mut reps, mut filter) =
        (None, None, DEFAULT_REPETITIONS, BTreeSet::new());
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixture" => fixture = args.next(),
            "--sha256" => sha = args.next(),
            "--repetitions" => {
                reps = args.next().ok_or("--repetitions needs a value")?.parse()?;
                if !(1..=MAX_REPETITIONS).contains(&reps) {
                    return Err(format!("repetitions must be 1..={MAX_REPETITIONS}").into());
                }
            }
            "--scenario" => {
                filter.extend(
                    args.next()
                        .ok_or("--scenario needs a value")?
                        .split(',')
                        .map(str::to_owned),
                );
            }
            "--help" => {
                println!("--repetitions 1..=100 --scenario ID[,ID] --fixture PATH --sha256 HEX");
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {a}").into()),
        }
    }
    let (sg, sm) = synthetic();
    let known = scenarios(&sg, &sm)
        .into_iter()
        .map(|s| s.id)
        .collect::<BTreeSet<_>>();
    if filter.iter().any(|s| !known.contains(s)) {
        return Err("unknown scenario id".into());
    }
    if let Some(p) = fixture {
        let want = sha.ok_or("real fixture requires --sha256")?;
        if want.len() != 64 || !want.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("--sha256 must be 64 hex digits".into());
        }
        let original = file_digest(Path::new(&p))?;
        if !original.eq_ignore_ascii_case(&want) {
            return Err(format!("fixture SHA-256 mismatch: {original}").into());
        }
        let dir = std::env::temp_dir().join(format!("freshness-perf-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let _guard = Scratch(dir.clone());
        let copy = dir.join("fixture.rel");
        fs::copy(&p, &copy)?;
        let mut perms = fs::metadata(&copy)?.permissions();
        perms.set_readonly(false);
        fs::set_permissions(&copy, perms)?;
        if file_digest(&copy)? != original {
            return Err("disposable copy hash mismatch".into());
        }
        let cva = Cva::open(&copy)?;
        let snap = cva.community_snapshot();
        let valid = snap.as_ref().is_some_and(|x| {
            cva.community_stats().current && x.derived_graph_version == cva.memory_graph_version()
        });
        let mut g = MemoryAdjacency::new();
        for r in cva.graph_relations().into_iter().filter(|x| x.active) {
            add_edge(&mut g, r.source, r.target)
        }
        let mut m = CommunityMembership::new();
        if valid {
            for c in &snap.as_ref().unwrap().communities {
                for id in &c.members {
                    m.insert(*id, c.id);
                }
            }
        }
        let label = Path::new(&p)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rel");
        let mut out = run_all(
            label,
            &g,
            &m,
            &filter,
            reps,
            Some(cva.memory_graph_version()),
            snap.as_ref().map(|s| s.generation),
            snap.as_ref().map(|s| s.derived_graph_version),
        )?;
        out["fixture_sha256"] = json!(original);
        out["source_hash_unchanged"] = json!(file_digest(Path::new(&p))? == original);
        out["owner_open_coldness"] =
            json!("cold process only; copy and SHA-256 reads warm the operating-system file cache");
        out["topology_reuse"] =
            json!("one pinned in-memory graph and Community projection reused across repetitions");
        out["graph"]["snapshot_validity"] = json!(if valid {
            "current"
        } else {
            "stale_or_missing_membership_treated_unknown"
        });
        if !out["source_hash_unchanged"].as_bool().unwrap_or(false) {
            return Err("authoritative source hash changed".into());
        }
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    } else if sha.is_some() {
        return Err("--sha256 requires --fixture".into());
    }
    let (g, m) = synthetic();
    let mut out = run_all(
        "synthetic_dense_local_v1",
        &g,
        &m,
        &filter,
        reps,
        None,
        None,
        None,
    )?;
    out["graph"]["snapshot_validity"] = json!("synthetic_frozen_membership");
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
