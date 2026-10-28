use super::*;

const ROUTE_K: usize = 4;

#[derive(Default)]
struct QualityTotals {
    queries: usize,
    support_total: usize,
    global_hits: [usize; BUDGETS.len()],
    tiered_hits: [usize; BUDGETS.len()],
    global_dominant: [f64; BUDGETS.len()],
    tiered_dominant: [f64; BUDGETS.len()],
    global_effective: [f64; BUDGETS.len()],
    tiered_effective: [f64; BUDGETS.len()],
    global_redundancy: [f64; BUDGETS.len()],
    tiered_redundancy: [f64; BUDGETS.len()],
    global_query_similarity: [f64; BUDGETS.len()],
    tiered_query_similarity: [f64; BUDGETS.len()],
    global_words: [usize; BUDGETS.len()],
    tiered_words: [usize; BUDGETS.len()],
}

#[test]
fn community_end_to_end_context_quality_benchmark() {
    let fixture = load_fixture();
    let graph = adjacency(&fixture);
    let mut totals = QualityTotals::default();

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

        evaluate_fold(&fixture, &graph, &held_out, &router, &mut totals);
    }

    println!(
        "budget,global_support,tiered_support,global_dominant_share,tiered_dominant_share,global_effective_communities,tiered_effective_communities,global_redundancy,tiered_redundancy,global_query_similarity,tiered_query_similarity,global_words,tiered_words"
    );
    let queries = totals.queries.max(1) as f64;
    for (slot, budget) in BUDGETS.into_iter().enumerate() {
        println!(
            "{budget},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2}",
            totals.global_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.tiered_hits[slot] as f64 / totals.support_total.max(1) as f64,
            totals.global_dominant[slot] / queries,
            totals.tiered_dominant[slot] / queries,
            totals.global_effective[slot] / queries,
            totals.tiered_effective[slot] / queries,
            totals.global_redundancy[slot] / queries,
            totals.tiered_redundancy[slot] / queries,
            totals.global_query_similarity[slot] / queries,
            totals.tiered_query_similarity[slot] / queries,
            totals.global_words[slot] as f64 / queries,
            totals.tiered_words[slot] as f64 / queries,
        );
    }
}

fn evaluate_fold(
    fixture: &crate::community_routing_bench_fixture::RoutingFixture,
    graph: &HashMap<MemoryId, Vec<MemoryId>>,
    held_out: &HashSet<MemoryId>,
    router: &[reliquary_retrieval::community_routing::CommunitySubcentroid],
    totals: &mut QualityTotals,
) {
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
        let global_seeds = global_seeds(&fixture.vectors, probe.query, query, SEEDS);
        let selected: HashSet<_> = route_subcentroids(router, query, ROUTE_K)
            .into_iter()
            .collect();
        let candidates: Vec<_> = fixture
            .vectors
            .keys()
            .copied()
            .filter(|id| *id != probe.query)
            .filter(|id| target_admitted(&fixture.memberships, *id, &selected))
            .collect();
        let tiered_seeds = fine_seeds(&fixture.vectors, query, &candidates, SEEDS);

        totals.queries += 1;
        totals.support_total += support.len();
        for (slot, budget) in BUDGETS.into_iter().enumerate() {
            let global = traverse(
                fixture,
                graph,
                &global_seeds,
                TraversalPolicy::Graph,
                budget,
            );
            let tiered = traverse(
                fixture,
                graph,
                &tiered_seeds,
                TraversalPolicy::CommunityWithinDepth,
                budget,
            );
            totals.global_hits[slot] += support
                .iter()
                .filter(|id| global.visited.contains(id))
                .count();
            totals.tiered_hits[slot] += support
                .iter()
                .filter(|id| tiered.visited.contains(id))
                .count();
            totals.global_words[slot] += global.tokens;
            totals.tiered_words[slot] += tiered.tokens;

            accumulate_quality(fixture, query, &global.visited, totals, slot, false);
            accumulate_quality(fixture, query, &tiered.visited, totals, slot, true);
        }
    }
}

fn accumulate_quality(
    fixture: &crate::community_routing_bench_fixture::RoutingFixture,
    query: &[f32],
    visited: &HashSet<MemoryId>,
    totals: &mut QualityTotals,
    slot: usize,
    tiered: bool,
) {
    let quality = context_quality(fixture, query, visited);
    let (dominant, effective, redundancy, query_similarity) = if tiered {
        (
            &mut totals.tiered_dominant,
            &mut totals.tiered_effective,
            &mut totals.tiered_redundancy,
            &mut totals.tiered_query_similarity,
        )
    } else {
        (
            &mut totals.global_dominant,
            &mut totals.global_effective,
            &mut totals.global_redundancy,
            &mut totals.global_query_similarity,
        )
    };
    dominant[slot] += quality.0;
    effective[slot] += quality.1;
    redundancy[slot] += quality.2;
    query_similarity[slot] += quality.3;
}

fn context_quality(
    fixture: &crate::community_routing_bench_fixture::RoutingFixture,
    query: &[f32],
    visited: &HashSet<MemoryId>,
) -> (f64, f64, f64, f64) {
    let mut counts = HashMap::<CommunityId, usize>::new();
    let vectors: Vec<_> = visited
        .iter()
        .filter_map(|id| {
            if let Some(community) = fixture.memberships.get(id) {
                *counts.entry(*community).or_default() += 1;
            }
            fixture.vectors.get(id)
        })
        .collect();
    let assigned = counts.values().sum::<usize>() as f64;
    let dominant = counts.values().copied().max().unwrap_or(0) as f64 / assigned.max(1.0);
    let entropy = counts
        .values()
        .map(|count| *count as f64 / assigned.max(1.0))
        .filter(|p| *p > 0.0)
        .map(|p| -p * p.ln())
        .sum::<f64>();
    let effective = if counts.is_empty() {
        0.0
    } else {
        entropy.exp()
    };
    let redundancy = mean_pairwise_cosine(&vectors);
    let query_similarity = vectors
        .iter()
        .filter_map(|vector| cosine(query, vector))
        .sum::<f64>()
        / vectors.len().max(1) as f64;
    (dominant, effective, redundancy, query_similarity)
}

fn mean_pairwise_cosine(vectors: &[&Vec<f32>]) -> f64 {
    let Some(dimensions) = vectors.first().map(|vector| vector.len()) else {
        return 0.0;
    };
    let mut sum = vec![0.0_f64; dimensions];
    let mut valid = 0_usize;
    for vector in vectors {
        let norm = vector
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt();
        if norm == 0.0 {
            continue;
        }
        for (target, value) in sum.iter_mut().zip(vector.iter()) {
            *target += f64::from(*value) / norm;
        }
        valid += 1;
    }
    if valid < 2 {
        return 0.0;
    }
    let squared_sum_norm = sum.iter().map(|value| value * value).sum::<f64>();
    (squared_sum_norm - valid as f64) / (valid * (valid - 1)) as f64
}
