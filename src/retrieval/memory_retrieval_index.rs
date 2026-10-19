use crate::compatibility_vector::cosine;
use crate::{CommunityId, CommunitySnapshot, MemoryId};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommunitySubcentroid {
    pub(crate) community_id: CommunityId,
    pub(crate) routing_key: MemoryId,
    pub(crate) vector: Vec<f32>,
}

pub(crate) fn build_subcentroids(
    snapshot: &CommunitySnapshot,
    vectors: &HashMap<MemoryId, Vec<f32>>,
    count: usize,
) -> Vec<CommunitySubcentroid> {
    snapshot
        .communities
        .iter()
        .flat_map(|community| {
            let points: Vec<_> = community
                .members
                .iter()
                .filter_map(|member| vectors.get(member).cloned())
                .collect();
            subcentroids(&points, count)
                .into_iter()
                .map(|vector| CommunitySubcentroid {
                    community_id: community.id,
                    routing_key: *community
                        .members
                        .iter()
                        .min()
                        .expect("Community is nonempty"),
                    vector,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn route_communities(
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
    });
    ranked.truncate(limit.min(ranked.len()));
    ranked.into_iter().map(|(community, _)| community).collect()
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

#[cfg(test)]
mod rewrite_tests {
    use super::*;
    #[test]
    fn equal_similarity_routing_uses_membership_identity_across_owner_changes() {
        let first = MemoryId([1; 32]);
        let second = MemoryId([2; 32]);
        let before = [
            CommunitySubcentroid {
                community_id: CommunityId([8; 32]),
                routing_key: first,
                vector: vec![1.0, 0.0],
            },
            CommunitySubcentroid {
                community_id: CommunityId([3; 32]),
                routing_key: second,
                vector: vec![1.0, 0.0],
            },
        ];
        let after = [
            CommunitySubcentroid {
                community_id: CommunityId([2; 32]),
                ..before[0].clone()
            },
            CommunitySubcentroid {
                community_id: CommunityId([9; 32]),
                ..before[1].clone()
            },
        ];
        assert_eq!(
            route_communities(&before, &[1.0, 0.0], 1),
            vec![before[0].community_id]
        );
        assert_eq!(
            route_communities(&after, &[1.0, 0.0], 1),
            vec![after[0].community_id]
        );
    }
}
