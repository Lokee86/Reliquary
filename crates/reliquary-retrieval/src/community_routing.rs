use reliquary_graph::{CommunityId, CommunitySnapshot};
use reliquary_memory_core::MemoryId;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct CommunitySubcentroid {
    pub community_id: CommunityId,
    pub routing_key: MemoryId,
    pub vector: Vec<f32>,
}

pub fn build_subcentroids(
    snapshot: &CommunitySnapshot,
    vectors: &HashMap<MemoryId, Vec<f32>>,
    count: usize,
) -> Vec<CommunitySubcentroid> {
    snapshot
        .communities
        .iter()
        .flat_map(|community| {
            let mut members = community.members.clone();
            members.sort();
            members.dedup();
            let points = members
                .iter()
                .filter_map(|member| vectors.get(&MemoryId(*member)).cloned())
                .collect::<Vec<_>>();
            let Some(routing_key) = community
                .members
                .iter()
                .map(|member| MemoryId(*member))
                .min()
            else {
                return Vec::new();
            };
            subcentroids(&points, count)
                .into_iter()
                .map(|vector| CommunitySubcentroid {
                    community_id: community.id,
                    routing_key,
                    vector,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn route_communities(
    entries: &[CommunitySubcentroid],
    query: &[f32],
    limit: usize,
) -> Vec<CommunityId> {
    let mut best = HashMap::<CommunityId, (f64, MemoryId)>::new();
    for entry in entries {
        if let Some(score) = cosine(query, &entry.vector) {
            best.entry(entry.community_id)
                .and_modify(|current| current.0 = current.0.max(score))
                .or_insert((score, entry.routing_key));
        }
    }
    let mut ranked: Vec<_> = best.into_iter().collect();
    ranked.sort_by(|left, right| {
        right
            .1
            .0
            .total_cmp(&left.1.0)
            .then_with(|| left.1.1.cmp(&right.1.1))
            .then_with(|| left.0.cmp(&right.0))
    });
    ranked.truncate(limit.min(ranked.len()));
    ranked.into_iter().map(|(id, _)| id).collect()
}

fn subcentroids(points: &[Vec<f32>], count: usize) -> Vec<Vec<f32>> {
    let Some(center) = mean_vector(points.iter()) else {
        return Vec::new();
    };
    let target = count.min(points.len()).max(1);
    let first = points
        .iter()
        .enumerate()
        .max_by(|left, right| {
            cosine(&center, left.1)
                .unwrap_or(f64::NEG_INFINITY)
                .total_cmp(&cosine(&center, right.1).unwrap_or(f64::NEG_INFINITY))
        })
        .map(|(index, _)| index)
        .unwrap();
    let mut seeds = vec![points[first].clone()];
    while seeds.len() < target {
        let next = points
            .iter()
            .min_by(|left, right| {
                nearest_similarity(left, &seeds).total_cmp(&nearest_similarity(right, &seeds))
            })
            .unwrap()
            .clone();
        seeds.push(next);
    }
    for _ in 0..2 {
        let mut groups = vec![Vec::<&Vec<f32>>::new(); seeds.len()];
        for point in points {
            let index = seeds
                .iter()
                .enumerate()
                .max_by(|left, right| {
                    cosine(point, left.1)
                        .unwrap_or(f64::NEG_INFINITY)
                        .total_cmp(&cosine(point, right.1).unwrap_or(f64::NEG_INFINITY))
                })
                .map(|(index, _)| index)
                .unwrap();
            groups[index].push(point);
        }
        for (seed, group) in seeds.iter_mut().zip(groups) {
            if let Some(center) = mean_vector(group.into_iter()) {
                *seed = center;
            }
        }
    }
    seeds
}

fn nearest_similarity(point: &[f32], seeds: &[Vec<f32>]) -> f64 {
    seeds
        .iter()
        .filter_map(|seed| cosine(point, seed))
        .fold(f64::NEG_INFINITY, f64::max)
}

fn mean_vector<'a>(vectors: impl Iterator<Item = &'a Vec<f32>>) -> Option<Vec<f32>> {
    let vectors: Vec<_> = vectors.collect();
    let dimensions = vectors.first()?.len();
    let mut output = vec![0.0_f32; dimensions];
    for vector in &vectors {
        if vector.len() != dimensions || vector.iter().any(|value| !value.is_finite()) {
            return None;
        }
        for (target, value) in output.iter_mut().zip(vector.iter()) {
            *target += *value;
        }
    }
    let scale = 1.0 / vectors.len() as f32;
    for value in &mut output {
        *value *= scale;
    }
    Some(output)
}

fn cosine(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (&a, &b) in left.iter().zip(right) {
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let (a, b) = (f64::from(a), f64::from(b));
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    (left_norm > 0.0 && right_norm > 0.0)
        .then(|| (dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_route_ties_use_stable_membership_key() {
        let first = CommunityId([8; 32]);
        let second = CommunityId([3; 32]);
        let entries = [
            CommunitySubcentroid {
                community_id: first,
                routing_key: MemoryId([1; 32]),
                vector: vec![1.0, 0.0],
            },
            CommunitySubcentroid {
                community_id: second,
                routing_key: MemoryId([2; 32]),
                vector: vec![1.0, 0.0],
            },
        ];
        assert_eq!(route_communities(&entries, &[1.0, 0.0], 1), vec![first]);
    }
}
