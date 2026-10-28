use crate::community_routing::cosine;
use crate::community_routing_bench_fixture::load_fixture;
use crate::community_traversal_bench_fixture::{adjacency, ranked_targets};
use crate::community_traversal_bench_support::{TraversalPolicy, traverse};
use crate::{CommunityId, MemoryId};
use reliquary_retrieval::community_routing::{
    build_subcentroids, route_communities as route_subcentroids,
};
use std::collections::{HashMap, HashSet};

const ROUTE_K: usize = 5;
const SEEDS: usize = 4;
const BUDGETS: [usize; 3] = [16, 32, 64];

#[derive(Default)]
struct Totals {
    queries: usize,
    route_hits: usize,
    route_targets: usize,
    seed_hits: usize,
    support_total: usize,
    global_support_hits: [usize; 3],
    tiered_support_hits: [usize; 3],
    fine_candidates: usize,
    global_candidates: usize,
    tiered_edges: [usize; 3],
    tiered_communities: [usize; 3],
    tiered_crossings: [usize; 3],
    tiered_tokens: [usize; 3],
}

#[test]
fn community_retrieval_traversal_end_to_end_benchmark() {
    let fixture = load_fixture();
    let held_out: HashSet<_> = fixture
        .vectors
        .keys()
        .copied()
        .filter(|id| id.0[0] % 5 == 0)
        .collect();
    let training: HashMap<_, _> = fixture
        .vectors
        .iter()
        .filter(|(id, _)| !held_out.contains(id))
        .map(|(id, vector)| (*id, vector.clone()))
        .collect();
    let lower_snapshot = fixture.snapshot.clone().into();
    let router = build_subcentroids(&lower_snapshot, &training, 4);
    let graph = adjacency(&fixture);
    let mut totals = Totals::default();

    for probe in fixture
        .semantic
        .iter()
        .filter(|probe| held_out.contains(&probe.query))
    {
        let Some(support) = graph.get(&probe.query) else {
            continue;
        };
        if support.is_empty() {
            continue;
        }
        let query = fixture.vectors.get(&probe.query).unwrap();
        let baseline = ranked_targets(&fixture, probe.query, &probe.targets);
        let global_seeds: Vec<_> = baseline.iter().copied().take(SEEDS).collect();
        let selected: HashSet<_> = route_subcentroids(&router, query, ROUTE_K)
            .into_iter()
            .collect();
        totals.route_hits += probe
            .targets
            .iter()
            .filter(|target| target_admitted(&fixture.memberships, **target, &selected))
            .count();
        totals.route_targets += probe.targets.len();

        let candidates: Vec<_> = fixture
            .vectors
            .keys()
            .copied()
            .filter(|id| *id != probe.query)
            .filter(|id| target_admitted(&fixture.memberships, *id, &selected))
            .collect();
        let tiered_seeds = fine_seeds(&fixture.vectors, query, &candidates, SEEDS);
        totals.seed_hits += tiered_seeds
            .iter()
            .filter(|seed| global_seeds.contains(seed))
            .count();
        totals.fine_candidates += candidates.len();
        totals.global_candidates += fixture.vectors.len().saturating_sub(1);
        totals.support_total += support.len();
        totals.queries += 1;

        for (slot, budget) in BUDGETS.into_iter().enumerate() {
            let global = traverse(
                &fixture,
                &graph,
                &global_seeds,
                TraversalPolicy::Graph,
                budget,
            );
            let tiered = traverse(
                &fixture,
                &graph,
                &tiered_seeds,
                TraversalPolicy::CommunityWithinDepth,
                budget,
            );
            totals.global_support_hits[slot] += support
                .iter()
                .filter(|id| global.visited.contains(id))
                .count();
            totals.tiered_support_hits[slot] += support
                .iter()
                .filter(|id| tiered.visited.contains(id))
                .count();
            totals.tiered_edges[slot] += tiered.edges;
            totals.tiered_communities[slot] += tiered.communities;
            totals.tiered_crossings[slot] += tiered.boundary_crossings;
            totals.tiered_tokens[slot] += tiered.tokens;
        }
    }

    let q = totals.queries.max(1) as f64;
    let route_recall = totals.route_hits as f64 / totals.route_targets.max(1) as f64;
    let seed_overlap = totals.seed_hits as f64 / (totals.queries.max(1) * SEEDS) as f64;
    let fine_admit = totals.fine_candidates as f64 / totals.global_candidates.max(1) as f64;
    let total_work = (totals.fine_candidates + totals.queries * router.len()) as f64
        / totals.global_candidates.max(1) as f64;
    println!(
        "queries={} router_reps={} route_recall={route_recall:.4} seed_overlap={seed_overlap:.4} fine_admit={fine_admit:.4} total_vector_work={total_work:.4}",
        totals.queries,
        router.len()
    );
    println!(
        "budget,global_support_recall,tiered_support_recall,tiered_edges,tiered_communities,tiered_crossings,tiered_tokens"
    );
    for (slot, budget) in BUDGETS.into_iter().enumerate() {
        println!(
            "{budget},{:.4},{:.4},{:.2},{:.2},{:.2},{:.1}",
            totals.global_support_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.tiered_support_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.tiered_edges[slot] as f64 / q,
            totals.tiered_communities[slot] as f64 / q,
            totals.tiered_crossings[slot] as f64 / q,
            totals.tiered_tokens[slot] as f64 / q,
        );
    }
}

fn target_admitted(
    memberships: &HashMap<MemoryId, CommunityId>,
    target: MemoryId,
    selected: &HashSet<CommunityId>,
) -> bool {
    memberships
        .get(&target)
        .is_none_or(|community| selected.contains(community))
}

fn fine_seeds(
    vectors: &HashMap<MemoryId, Vec<f32>>,
    query: &[f32],
    candidates: &[MemoryId],
    limit: usize,
) -> Vec<MemoryId> {
    let mut ranked: Vec<_> = candidates
        .iter()
        .filter_map(|id| cosine(query, vectors.get(id)?).map(|score| (*id, score)))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
    ranked.into_iter().take(limit).map(|(id, _)| id).collect()
}
