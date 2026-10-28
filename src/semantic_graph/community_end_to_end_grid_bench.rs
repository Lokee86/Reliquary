use crate::community_routing::cosine;
use crate::community_routing_bench_fixture::load_fixture;
use crate::community_traversal_bench_fixture::adjacency;
use crate::community_traversal_bench_support::{TraversalPolicy, traverse};
use crate::{CommunityId, MemoryId};
use reliquary_retrieval::community_routing::{
    CommunitySubcentroid, build_subcentroids, route_communities as route_subcentroids,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const FOLDS: usize = 5;
const ROUTE_KS: [usize; 3] = [2, 3, 4];
const BUDGETS: [usize; 3] = [16, 32, 64];
const SEEDS: usize = 4;

#[path = "community_end_to_end_quality_bench.rs"]
mod quality;

#[derive(Default)]
struct Totals {
    queries: usize,
    route_hits: usize,
    route_targets: usize,
    seed_hits: usize,
    route_candidates: usize,
    fine_candidates: usize,
    global_candidates: usize,
    support_total: usize,
    global_hits: [usize; BUDGETS.len()],
    tiered_hits: [usize; BUDGETS.len()],
    global_nodes: [usize; BUDGETS.len()],
    tiered_nodes: [usize; BUDGETS.len()],
    global_edges: [usize; BUDGETS.len()],
    tiered_edges: [usize; BUDGETS.len()],
    global_communities: [usize; BUDGETS.len()],
    tiered_communities: [usize; BUDGETS.len()],
    global_crossings: [usize; BUDGETS.len()],
    tiered_crossings: [usize; BUDGETS.len()],
    global_tokens: [usize; BUDGETS.len()],
    tiered_tokens: [usize; BUDGETS.len()],
    route_ns: u128,
    fine_ns: u128,
    global_search_ns: u128,
    global_traversal_ns: [u128; BUDGETS.len()],
    tiered_traversal_ns: [u128; BUDGETS.len()],
}

impl Totals {
    fn add(&mut self, other: &Self) {
        self.queries += other.queries;
        self.route_hits += other.route_hits;
        self.route_targets += other.route_targets;
        self.seed_hits += other.seed_hits;
        self.route_candidates += other.route_candidates;
        self.fine_candidates += other.fine_candidates;
        self.global_candidates += other.global_candidates;
        self.support_total += other.support_total;
        self.route_ns += other.route_ns;
        self.fine_ns += other.fine_ns;
        self.global_search_ns += other.global_search_ns;
        for slot in 0..BUDGETS.len() {
            self.global_hits[slot] += other.global_hits[slot];
            self.tiered_hits[slot] += other.tiered_hits[slot];
            self.global_nodes[slot] += other.global_nodes[slot];
            self.tiered_nodes[slot] += other.tiered_nodes[slot];
            self.global_edges[slot] += other.global_edges[slot];
            self.tiered_edges[slot] += other.tiered_edges[slot];
            self.global_communities[slot] += other.global_communities[slot];
            self.tiered_communities[slot] += other.tiered_communities[slot];
            self.global_crossings[slot] += other.global_crossings[slot];
            self.tiered_crossings[slot] += other.tiered_crossings[slot];
            self.global_tokens[slot] += other.global_tokens[slot];
            self.tiered_tokens[slot] += other.tiered_tokens[slot];
            self.global_traversal_ns[slot] += other.global_traversal_ns[slot];
            self.tiered_traversal_ns[slot] += other.tiered_traversal_ns[slot];
        }
    }
}

#[test]
fn community_end_to_end_operating_point_benchmark() {
    let fixture = load_fixture();
    let graph = adjacency(&fixture);
    let mut aggregate: Vec<_> = ROUTE_KS.iter().map(|_| Totals::default()).collect();

    println!(
        "scope,fold,k,queries,route_recall,seed_overlap,vector_work,budget,global_support,tiered_support,global_search_us,route_us,fine_us,global_traversal_us,tiered_traversal_us,global_nodes,tiered_nodes,global_edges,tiered_edges,global_communities,tiered_communities,global_crossings,tiered_crossings,global_tokens,tiered_tokens"
    );

    for fold in 0..FOLDS {
        let held_out: HashSet<_> = fixture
            .vectors
            .keys()
            .copied()
            .filter(|id| fold_for(*id) == fold)
            .collect();
        let training: HashMap<_, _> = fixture
            .vectors
            .iter()
            .filter(|(id, _)| !held_out.contains(id))
            .map(|(id, vector)| (*id, vector.clone()))
            .collect();
        let lower_snapshot = fixture.snapshot.clone().into();
        let router = build_subcentroids(&lower_snapshot, &training, 4);

        for (k_slot, k) in ROUTE_KS.into_iter().enumerate() {
            let totals = evaluate_fold(&fixture, &graph, &held_out, &router, k);
            print_rows("fold", fold, k, &totals);
            aggregate[k_slot].add(&totals);
        }
    }

    for (k_slot, k) in ROUTE_KS.into_iter().enumerate() {
        print_rows("aggregate", FOLDS, k, &aggregate[k_slot]);
    }
}

fn evaluate_fold(
    fixture: &crate::community_routing_bench_fixture::RoutingFixture,
    graph: &HashMap<MemoryId, Vec<MemoryId>>,
    held_out: &HashSet<MemoryId>,
    router: &[CommunitySubcentroid],
    k: usize,
) -> Totals {
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

        let started = Instant::now();
        let global_seeds = global_seeds(&fixture.vectors, probe.query, query, SEEDS);
        totals.global_search_ns += started.elapsed().as_nanos();

        let started = Instant::now();
        let selected: HashSet<_> = route_subcentroids(router, query, k).into_iter().collect();
        totals.route_ns += started.elapsed().as_nanos();

        totals.route_hits += probe
            .targets
            .iter()
            .filter(|target| target_admitted(&fixture.memberships, **target, &selected))
            .count();
        totals.route_targets += probe.targets.len();

        let started = Instant::now();
        let candidates: Vec<_> = fixture
            .vectors
            .keys()
            .copied()
            .filter(|id| *id != probe.query)
            .filter(|id| target_admitted(&fixture.memberships, *id, &selected))
            .collect();
        let tiered_seeds = fine_seeds(&fixture.vectors, query, &candidates, SEEDS);
        totals.fine_ns += started.elapsed().as_nanos();

        totals.seed_hits += tiered_seeds
            .iter()
            .filter(|seed| global_seeds.contains(seed))
            .count();
        totals.route_candidates += router.len();
        totals.fine_candidates += candidates.len();
        totals.global_candidates += fixture.vectors.len().saturating_sub(1);
        totals.support_total += support.len();
        totals.queries += 1;

        for (slot, budget) in BUDGETS.into_iter().enumerate() {
            let started = Instant::now();
            let global = traverse(
                fixture,
                graph,
                &global_seeds,
                TraversalPolicy::Graph,
                budget,
            );
            totals.global_traversal_ns[slot] += started.elapsed().as_nanos();

            let started = Instant::now();
            let tiered = traverse(
                fixture,
                graph,
                &tiered_seeds,
                TraversalPolicy::CommunityWithinDepth,
                budget,
            );
            totals.tiered_traversal_ns[slot] += started.elapsed().as_nanos();

            totals.global_hits[slot] += support
                .iter()
                .filter(|id| global.visited.contains(id))
                .count();
            totals.tiered_hits[slot] += support
                .iter()
                .filter(|id| tiered.visited.contains(id))
                .count();
            totals.global_nodes[slot] += global.visited.len();
            totals.tiered_nodes[slot] += tiered.visited.len();
            totals.global_edges[slot] += global.edges;
            totals.tiered_edges[slot] += tiered.edges;
            totals.global_communities[slot] += global.communities;
            totals.tiered_communities[slot] += tiered.communities;
            totals.global_crossings[slot] += global.boundary_crossings;
            totals.tiered_crossings[slot] += tiered.boundary_crossings;
            totals.global_tokens[slot] += global.tokens;
            totals.tiered_tokens[slot] += tiered.tokens;
        }
    }

    totals
}

fn print_rows(scope: &str, fold: usize, k: usize, totals: &Totals) {
    let queries = totals.queries.max(1) as f64;
    let route_recall = totals.route_hits as f64 / totals.route_targets.max(1) as f64;
    let seed_overlap = totals.seed_hits as f64 / (totals.queries.max(1) * SEEDS) as f64;
    let vector_work = (totals.route_candidates + totals.fine_candidates) as f64
        / totals.global_candidates.max(1) as f64;
    let global_search_us = totals.global_search_ns as f64 / queries / 1000.0;
    let route_us = totals.route_ns as f64 / queries / 1000.0;
    let fine_us = totals.fine_ns as f64 / queries / 1000.0;

    for (slot, budget) in BUDGETS.into_iter().enumerate() {
        println!(
            "{scope},{fold},{k},{},{route_recall:.4},{seed_overlap:.4},{vector_work:.4},{budget},{:.4},{:.4},{global_search_us:.3},{route_us:.3},{fine_us:.3},{:.3},{:.3},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            totals.queries,
            totals.global_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.tiered_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.global_traversal_ns[slot] as f64 / queries / 1000.0,
            totals.tiered_traversal_ns[slot] as f64 / queries / 1000.0,
            totals.global_nodes[slot] as f64 / queries,
            totals.tiered_nodes[slot] as f64 / queries,
            totals.global_edges[slot] as f64 / queries,
            totals.tiered_edges[slot] as f64 / queries,
            totals.global_communities[slot] as f64 / queries,
            totals.tiered_communities[slot] as f64 / queries,
            totals.global_crossings[slot] as f64 / queries,
            totals.tiered_crossings[slot] as f64 / queries,
            totals.global_tokens[slot] as f64 / queries,
            totals.tiered_tokens[slot] as f64 / queries,
        );
    }
}

fn fold_for(id: MemoryId) -> usize {
    id.0[0] as usize % FOLDS
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

fn global_seeds(
    vectors: &HashMap<MemoryId, Vec<f32>>,
    query_id: MemoryId,
    query: &[f32],
    limit: usize,
) -> Vec<MemoryId> {
    let candidates: Vec<_> = vectors
        .keys()
        .copied()
        .filter(|id| *id != query_id)
        .collect();
    fine_seeds(vectors, query, &candidates, limit)
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
