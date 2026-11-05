use crate::community_routing::{build_subcentroids, route_communities};
use reliquary_graph::{CommunityId, CommunitySnapshot};
use reliquary_memory_core::MemoryId;
use std::collections::{HashMap, HashSet};

pub const DEFAULT_MEMORY_RETRIEVAL_COMMUNITIES: usize = 4;
pub const DEFAULT_MEMORY_RETRIEVAL_SEEDS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRetrievalMode {
    CommunityRouted,
    GlobalExact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRetrievalConfig {
    pub mode: MemoryRetrievalMode,
    pub community_limit: usize,
    pub seed_limit: usize,
}
impl Default for MemoryRetrievalConfig {
    fn default() -> Self {
        Self {
            mode: MemoryRetrievalMode::CommunityRouted,
            community_limit: DEFAULT_MEMORY_RETRIEVAL_COMMUNITIES,
            seed_limit: DEFAULT_MEMORY_RETRIEVAL_SEEDS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryRetrievalHit {
    pub memory_id: MemoryId,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryRetrievalResult {
    pub mode_used: MemoryRetrievalMode,
    pub used_global_fallback: bool,
    pub selected_communities: Vec<CommunityId>,
    pub seeds: Vec<MemoryRetrievalHit>,
    pub memories: Vec<MemoryId>,
    pub routing_vectors_scored: usize,
    pub memory_vectors_scored: usize,
    pub global_memory_vectors: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalPolicyError {
    InvalidConfig,
    InvalidQueryVector,
    EmptyPopulation,
    StaleSnapshot,
}

/// All semantic inputs are immutable owner-prepared values; the evaluator owns no store.
pub struct MemoryRetrievalSnapshot {
    pub graph_version: u64,
    pub community_snapshot: Option<CommunitySnapshot>,
    pub vectors: HashMap<MemoryId, Vec<f32>>,
    pub searchable: HashSet<MemoryId>,
}

pub fn retrieve_memories(
    snapshot: &MemoryRetrievalSnapshot,
    query: &[f32],
    config: MemoryRetrievalConfig,
    current_graph_version: u64,
) -> Result<MemoryRetrievalResult, RetrievalPolicyError> {
    if config.seed_limit == 0 || config.community_limit == 0 {
        return Err(RetrievalPolicyError::InvalidConfig);
    }
    if query.is_empty() || query.iter().any(|v| !v.is_finite()) || norm(query).is_none() {
        return Err(RetrievalPolicyError::InvalidQueryVector);
    }
    if snapshot.graph_version != current_graph_version {
        return Err(RetrievalPolicyError::StaleSnapshot);
    }
    let dimensions = snapshot
        .vectors
        .values()
        .next()
        .map(Vec::len)
        .ok_or(RetrievalPolicyError::EmptyPopulation)?;
    if dimensions == 0
        || query.len() != dimensions
        || snapshot
            .vectors
            .values()
            .any(|v| v.len() != dimensions || norm(v).is_none())
    {
        return Err(RetrievalPolicyError::InvalidQueryVector);
    }
    let current = snapshot
        .community_snapshot
        .as_ref()
        .filter(|c| c.graph_version == snapshot.graph_version);
    let memberships: HashMap<_, _> = current
        .into_iter()
        .flat_map(|c| {
            c.communities.iter().flat_map(|community| {
                community
                    .members
                    .iter()
                    .map(move |m| (MemoryId(*m), community.id))
            })
        })
        .collect();
    let centroids = current
        .map(|c| build_subcentroids(c, &snapshot.vectors, 4))
        .unwrap_or_default();
    let mut mode_used = config.mode;
    let mut fallback = false;
    let mut selected_communities = Vec::new();
    let candidate_ids: Vec<MemoryId> =
        if config.mode == MemoryRetrievalMode::CommunityRouted && !centroids.is_empty() {
            selected_communities = route_communities(&centroids, query, config.community_limit);
            if selected_communities.is_empty() {
                mode_used = MemoryRetrievalMode::GlobalExact;
                fallback = true;
                sorted_ids(&snapshot.vectors)
            } else {
                let selected: HashSet<_> = selected_communities.iter().copied().collect();
                let mut ids: Vec<_> = snapshot
                    .vectors
                    .keys()
                    .copied()
                    .filter(|id| memberships.get(id).is_none_or(|c| selected.contains(c)))
                    .collect();
                ids.sort();
                ids
            }
        } else {
            if config.mode == MemoryRetrievalMode::CommunityRouted {
                mode_used = MemoryRetrievalMode::GlobalExact;
                fallback = true;
            }
            sorted_ids(&snapshot.vectors)
        };
    let mut seeds = score_ids(query, &snapshot.vectors, &candidate_ids)?;
    seeds.truncate(config.seed_limit);
    let memories = seeds
        .iter()
        .map(|hit| hit.memory_id)
        .filter(|id| snapshot.searchable.contains(id))
        .collect();
    Ok(MemoryRetrievalResult {
        mode_used,
        used_global_fallback: fallback,
        selected_communities,
        seeds,
        memories,
        routing_vectors_scored: centroids.len(),
        memory_vectors_scored: candidate_ids.len(),
        global_memory_vectors: snapshot.vectors.len(),
    })
}

fn sorted_ids(vectors: &HashMap<MemoryId, Vec<f32>>) -> Vec<MemoryId> {
    let mut ids: Vec<_> = vectors.keys().copied().collect();
    ids.sort();
    ids
}
fn score_ids(
    query: &[f32],
    vectors: &HashMap<MemoryId, Vec<f32>>,
    ids: &[MemoryId],
) -> Result<Vec<MemoryRetrievalHit>, RetrievalPolicyError> {
    let query_norm = norm(query).ok_or(RetrievalPolicyError::InvalidQueryVector)?;
    let mut hits = Vec::new();
    for id in ids {
        let vector = vectors.get(id).ok_or(RetrievalPolicyError::StaleSnapshot)?;
        let vector_norm = norm(vector).ok_or(RetrievalPolicyError::InvalidQueryVector)?;
        if vector.len() != query.len() {
            return Err(RetrievalPolicyError::InvalidQueryVector);
        }
        let dot: f64 = query
            .iter()
            .zip(vector)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum();
        let score = (dot / (query_norm * vector_norm)).clamp(-1.0, 1.0);
        if score > 0.0 && score.is_finite() {
            hits.push(MemoryRetrievalHit {
                memory_id: *id,
                score,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.memory_id.cmp(&b.memory_id))
    });
    Ok(hits)
}
fn norm(vector: &[f32]) -> Option<f64> {
    if vector.is_empty() || vector.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let value = vector
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    (value > 0.0 && value.is_finite()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_graph::{Community, CommunitySnapshot};

    fn id(value: u8) -> MemoryId {
        MemoryId([value; 32])
    }
    fn snapshot() -> MemoryRetrievalSnapshot {
        MemoryRetrievalSnapshot {
            graph_version: 4,
            community_snapshot: Some(CommunitySnapshot {
                generation: 1,
                graph_version: 4,
                communities: vec![
                    Community {
                        id: CommunityId([8; 32]),
                        members: vec![[1; 32], [2; 32]],
                    },
                    Community {
                        id: CommunityId([9; 32]),
                        members: vec![[3; 32]],
                    },
                ],
            }),
            vectors: [
                (id(1), vec![1.0, 0.0]),
                (id(2), vec![0.8, 0.6]),
                (id(3), vec![0.0, 1.0]),
            ]
            .into(),
            searchable: [id(1), id(2), id(3)].into(),
        }
    }

    #[test]
    fn routed_snapshot_scores_only_selected_communities_and_ties_are_stable() {
        let result = retrieve_memories(
            &snapshot(),
            &[1.0, 0.0],
            MemoryRetrievalConfig {
                community_limit: 1,
                seed_limit: 10,
                ..MemoryRetrievalConfig::default()
            },
            4,
        )
        .unwrap();
        assert_eq!(result.selected_communities, vec![CommunityId([8; 32])]);
        assert_eq!(result.memory_vectors_scored, 2);
        assert_eq!(result.memories, vec![id(1), id(2)]);
    }

    #[test]
    fn missing_or_stale_community_snapshot_falls_back_to_global() {
        let mut facts = snapshot();
        facts.community_snapshot = None;
        let result =
            retrieve_memories(&facts, &[1.0, 0.0], MemoryRetrievalConfig::default(), 4).unwrap();
        assert!(result.used_global_fallback);
        assert_eq!(result.mode_used, MemoryRetrievalMode::GlobalExact);
        assert_eq!(result.memory_vectors_scored, 3);
    }

    #[test]
    fn stale_graph_and_bad_query_fail_closed() {
        assert_eq!(
            retrieve_memories(
                &snapshot(),
                &[1.0, 0.0],
                MemoryRetrievalConfig::default(),
                3
            )
            .unwrap_err(),
            RetrievalPolicyError::StaleSnapshot
        );
        assert_eq!(
            retrieve_memories(
                &snapshot(),
                &[0.0, 0.0],
                MemoryRetrievalConfig::default(),
                4
            )
            .unwrap_err(),
            RetrievalPolicyError::InvalidQueryVector
        );
    }

    #[test]
    fn routing_is_invariant_to_snapshot_and_vector_insertion_order() {
        let original = snapshot();
        let mut permuted = snapshot();
        let communities = &mut permuted.community_snapshot.as_mut().unwrap().communities;
        communities.reverse();
        for community in communities {
            community.members.reverse();
        }
        let mut rows: Vec<_> = permuted.vectors.drain().collect();
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        permuted.vectors.extend(rows);
        for query in [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]] {
            assert_eq!(
                retrieve_memories(&original, &query, MemoryRetrievalConfig::default(), 4),
                retrieve_memories(&permuted, &query, MemoryRetrievalConfig::default(), 4)
            );
        }
        permuted.community_snapshot.as_mut().unwrap().graph_version = 3;
        assert!(
            retrieve_memories(&permuted, &[1.0, 0.0], MemoryRetrievalConfig::default(), 4)
                .unwrap()
                .used_global_fallback
        );
    }
}
