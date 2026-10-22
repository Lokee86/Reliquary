use crate::community_routing::cosine;
use crate::dream_owner_vectors::load_memory_vectors;
use crate::{CommunityId, CommunitySnapshot, Cva, GraphRelation, Memory, MemoryId};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

pub(crate) struct RoutingFixture {
    pub(crate) vectors: HashMap<MemoryId, Vec<f32>>,
    pub(crate) memories: HashMap<MemoryId, Memory>,
    pub(crate) snapshot: CommunitySnapshot,
    pub(crate) relations: Vec<GraphRelation>,
    pub(crate) memberships: HashMap<MemoryId, CommunityId>,
    pub(crate) gold: Vec<RoutingProbe>,
    pub(crate) semantic: Vec<RoutingProbe>,
}

#[derive(Clone)]
pub(crate) struct RoutingProbe {
    pub(crate) query: MemoryId,
    pub(crate) targets: Vec<MemoryId>,
}

pub(crate) fn load_fixture() -> RoutingFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rel_path = std::env::var_os("RELIQUARY_ROUTING_REL")
        .map(PathBuf::from)
        .expect("set RELIQUARY_ROUTING_REL to the current owner-split Dream Project REL; there is intentionally no default fixture");
    let mut rel = Cva::open(&rel_path)
        .unwrap_or_else(|error| panic!("open routing REL {} failed: {error}", rel_path.display()));
    assert!(
        rel.owner_uuid().is_some(),
        "routing fixture requires a durable current owner"
    );

    let snapshot = rel.refresh_communities_leiden().unwrap();
    rel.sync().unwrap();
    let relations = rel.graph_relations();
    let profile = rel
        .compatibility_profiles()
        .into_iter()
        .next()
        .expect("fixture compatibility profile");
    let by_body = load_memory_vectors(
        &mut rel.container,
        &rel.memories,
        &rel.memory_vectors,
        &rel.packed_vectors,
        profile.id,
    )
    .unwrap();
    let mut vectors = HashMap::new();
    for id in rel.memories.current_ids() {
        let body_id = rel.memories.current_body_id(id).unwrap();
        if let Some(vector) = by_body.get(&body_id) {
            vectors.insert(id, vector.clone());
        }
    }

    let mut memories = HashMap::new();
    for id in rel.memory_ids() {
        memories.insert(id, rel.memory(id).unwrap());
    }

    let memberships = membership_map(&snapshot);
    let positive_pairs = positive_gold_pairs(root.join("corpus/dream-web-gold-v1.json"));
    let gold = pair_probes(&positive_pairs, &memberships, &vectors);
    let semantic = semantic_probes(&vectors, crate::DEFAULT_DREAM_CANDIDATE_LIMIT);
    RoutingFixture {
        vectors,
        memories,
        snapshot,
        relations,
        memberships,
        gold,
        semantic,
    }
}

pub(crate) fn membership_map(snapshot: &CommunitySnapshot) -> HashMap<MemoryId, CommunityId> {
    snapshot
        .communities
        .iter()
        .flat_map(|community| {
            community
                .members
                .iter()
                .map(move |member| (*member, community.id))
        })
        .collect()
}

fn positive_gold_pairs(path: PathBuf) -> Vec<(MemoryId, MemoryId)> {
    let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    value["final_pairs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["present"].as_bool() == Some(true))
        .map(|row| {
            (
                memory_id(row["left"].as_str().unwrap()),
                memory_id(row["right"].as_str().unwrap()),
            )
        })
        .collect()
}

fn pair_probes(
    pairs: &[(MemoryId, MemoryId)],
    memberships: &HashMap<MemoryId, CommunityId>,
    vectors: &HashMap<MemoryId, Vec<f32>>,
) -> Vec<RoutingProbe> {
    let mut targets = HashMap::<MemoryId, HashSet<MemoryId>>::new();
    for &(left, right) in pairs {
        for (query, target) in [(left, right), (right, left)] {
            if vectors.contains_key(&query)
                && vectors.contains_key(&target)
                && memberships.contains_key(&target)
            {
                targets.entry(query).or_default().insert(target);
            }
        }
    }
    probes(targets)
}

pub(crate) fn semantic_probes(
    vectors: &HashMap<MemoryId, Vec<f32>>,
    limit: usize,
) -> Vec<RoutingProbe> {
    let mut targets = HashMap::<MemoryId, HashSet<MemoryId>>::new();
    for (&query, query_vector) in vectors {
        let mut ranked: Vec<_> = vectors
            .iter()
            .filter(|(id, _)| **id != query)
            .filter_map(|(id, vector)| cosine(query_vector, vector).map(|score| (*id, score)))
            .collect();
        ranked.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.0.cmp(&right.0.0))
        });
        for (id, _) in ranked.into_iter().take(limit) {
            targets.entry(query).or_default().insert(id);
        }
    }
    probes(targets)
}

fn probes(targets: HashMap<MemoryId, HashSet<MemoryId>>) -> Vec<RoutingProbe> {
    let mut output: Vec<_> = targets
        .into_iter()
        .map(|(query, targets)| {
            let mut targets: Vec<_> = targets.into_iter().collect();
            targets.sort_by_key(|target| target.0);
            RoutingProbe { query, targets }
        })
        .collect();
    output.sort_by_key(|probe| probe.query.0);
    output
}

fn memory_id(hex: &str) -> MemoryId {
    assert_eq!(hex.len(), 64);
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    MemoryId(bytes)
}
