//! Prospective P5 breadth diagnostic on frozen graph topology.
//! Hash-checks authoritative RELs and opens only disposable verified copies.
//! Histograms describe event effects, never stored scores or historical trajectories.
use reliquary_memory::{
    CommunityId, Cva, MemoryId,
    freshness::{CommunityMembership, MemoryAdjacency, PropagationEvent, propagate_with_stats},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const GROUPS: u8 = 32;
const EACH: u8 = 64;
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

fn distances(g: &MemoryAdjacency, roots: &[MemoryId]) -> BTreeMap<MemoryId, usize> {
    let mut d = BTreeMap::new();
    let mut q = std::collections::VecDeque::new();
    for r in roots {
        if d.insert(*r, 0).is_none() {
            q.push_back(*r);
        }
    }
    while let Some(a) = q.pop_front() {
        let next = d[&a] + 1;
        if let Some(ns) = g.get(&a) {
            for b in ns {
                if !d.contains_key(b) {
                    d.insert(*b, next);
                    q.push_back(*b);
                }
            }
        }
    }
    d
}
fn run_all(
    label: &str,
    g: &MemoryAdjacency,
    m: &CommunityMembership,
    graph_version: Option<u64>,
    community_generation: Option<u64>,
    community_graph_version: Option<u64>,
) -> Result<Value, Box<dyn Error>> {
    let nodes = g
        .keys()
        .copied()
        .chain(g.values().flat_map(|ns| ns.iter().copied()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if nodes.is_empty() {
        return Err("empty graph".into());
    }
    let mut anchors = BTreeSet::new();
    for i in 0..16 {
        anchors.insert(nodes[i * (nodes.len() - 1) / 15]);
    }
    let mut by_degree = nodes.clone();
    by_degree.sort_by_key(|n| (g.get(n).map_or(0, BTreeSet::len), *n));
    for i in [0, by_degree.len() / 2, by_degree.len() - 1] {
        anchors.insert(by_degree[i]);
    }
    let mut rows = Vec::new();
    for (sample, anchor) in anchors.iter().enumerate() {
        let d = distances(g, &[*anchor]);
        let mut clustered = nodes.clone();
        clustered.sort_by_key(|n| (d.get(n).copied().unwrap_or(usize::MAX), *n));
        let pos = nodes.binary_search(anchor).unwrap();
        for n in [1usize, 2, 4, 8] {
            for layout in ["clustered", "dispersed"] {
                if n == 1 && layout == "dispersed" {
                    continue;
                }
                let roots = if layout == "clustered" {
                    clustered.iter().take(n).copied().collect::<Vec<_>>()
                } else {
                    (0..n)
                        .map(|i| nodes[(pos + i * nodes.len() / n) % nodes.len()])
                        .collect()
                };
                for principal in [25i16, 50] {
                    if principal == 25 && n != 1 {
                        continue;
                    }
                    for mode in ["available", "all_unknown"] {
                        if m.is_empty() && mode == "available" {
                            continue;
                        }
                        let empty = CommunityMembership::new();
                        let membership = if mode == "all_unknown" { &empty } else { m };
                        let event = PropagationEvent {
                            roots: roots.clone(),
                            principal,
                            excluded: BTreeSet::new(),
                        };
                        let out = propagate_with_stats(&event, g, membership, 1)?;
                        let hops = distances(g, &roots);
                        if membership.is_empty() {
                            let expected = hops
                                .iter()
                                .filter_map(|(id, hop)| {
                                    let effect = i64::from(principal) - 10 * (*hop as i64);
                                    (effect > 0).then_some((*id, effect as i16))
                                })
                                .collect::<BTreeMap<_, _>>();
                            if expected != out.candidates {
                                return Err("unknown-membership BFS oracle differs".into());
                            }
                        }
                        let mut hist = BTreeMap::<String, usize>::new();
                        for (id, strength) in &out.candidates {
                            *hist
                                .entry(format!("hop={};effect={}", hops[id], strength))
                                .or_default() += 1;
                        }
                        let mut groups = BTreeMap::<Option<CommunityId>, Vec<MemoryId>>::new();
                        for root in &roots {
                            groups
                                .entry(membership.get(root).copied())
                                .or_default()
                                .push(*root);
                        }
                        let mut merged = BTreeMap::<MemoryId, i16>::new();
                        let mut origins = Vec::new();
                        for (origin, rs) in groups {
                            let e = PropagationEvent {
                                roots: rs.clone(),
                                principal,
                                excluded: BTreeSet::new(),
                            };
                            let result = propagate_with_stats(&e, g, membership, 1)?;
                            let hd = distances(g, &rs);
                            let mut oh = BTreeMap::<String, usize>::new();
                            for (id, strength) in &result.candidates {
                                merged
                                    .entry(*id)
                                    .and_modify(|old| *old = (*old).max(*strength))
                                    .or_insert(*strength);
                                let relation = match (origin, membership.get(id)) {
                                    (None, _) => "unknown_origin",
                                    (Some(a), Some(b)) if a == *b => "same_origin_community",
                                    _ => "outside_or_unknown_recipient",
                                };
                                *oh.entry(format!(
                                    "hop={};effect={};recipient={}",
                                    hd[id], strength, relation
                                ))
                                .or_default() += 1;
                            }
                            origins.push(json!({"origin":origin.map(|c|id_hex(MemoryId(c.0))),"root_count":rs.len(),"recipient_count":result.candidates.len(),"histogram":oh}));
                        }
                        if merged != out.candidates {
                            return Err("origin-group reconstruction differs".into());
                        }
                        rows.push(json!({"sample":sample,"anchor_degree":g.get(anchor).map_or(0,BTreeSet::len),"principal":principal,
                            "root_count":roots.len(),"layout":layout,"membership_mode":mode,"roots":roots.iter().map(|r|id_hex(*r)).collect::<Vec<_>>(),
                            "recipient_count":out.candidates.len(),"touched_fraction":out.candidates.len() as f64/nodes.len() as f64,
                            "nearly_full_web":out.candidates.len()*10>=nodes.len()*9,"effect_hop_histogram":hist,"origin_groups":origins,
                            "examined_edges":out.stats.examined_edges,"edges_per_recipient":out.stats.examined_edges as f64/out.candidates.len().max(1) as f64,
                            "candidate_map_sha256":event_digest(&out.candidates),"origin_reconstruction_equal":true}));
                    }
                }
            }
        }
    }
    Ok(
        json!({"fixture":label,"graph":{"node_count":nodes.len(),"undirected_active_edges":edge_count(g),"community_membership_count":m.len(),
        "projection_sha256":projection_digest(g,m),"graph_version":graph_version,"community_generation":community_generation,"community_graph_version":community_graph_version},
        "interpretation":"Prospective topology-only breadth; no historical trajectory or production-frequency claims. Hop is shortest unweighted graph distance, not necessarily the winning cost path. Origin histograms overlap; canonical union uses maximum effect.",
        "sampling":"16 lexical quantile anchors plus minimum/median/maximum degree anchors, deduplicated; clustered nearest-hop roots and evenly spaced lexical dispersed roots; no inference or Community regeneration",
        "nearly_full_web_threshold":0.9,"anchor_count":anchors.len(),"results":rows}),
    )
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let (mut fixture, mut sha) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixture" => fixture = args.next(),
            "--sha256" => sha = args.next(),
            "--help" => {
                println!("--fixture PATH --sha256 HEX");
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {a}").into()),
        }
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
    let mut out = run_all("synthetic_dense_local_v1", &g, &m, None, None, None)?;
    out["graph"]["snapshot_validity"] = json!("synthetic_frozen_membership");
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
