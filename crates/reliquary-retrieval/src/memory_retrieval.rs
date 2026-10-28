use std::collections::{HashMap, HashSet};

use reliquary_graph::{COMMUNITY_ALGORITHM_VERSION, CommunityId, CommunitySnapshot};
use reliquary_memory_core::MemoryId;
use reliquary_vectors::CompatibilityProfileId;
use reliquary_vectors::compatibility_vector::cosine;

use crate::community_routing::{CommunitySubcentroid, build_subcentroids, route_communities};

pub const DEFAULT_MEMORY_RETRIEVAL_COMMUNITIES: usize = 4;
pub const DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS: usize = 4;
pub const DEFAULT_MEMORY_RETRIEVAL_SEEDS: usize = 4;
pub const DEFAULT_MEMORY_RETRIEVAL_BUDGET: usize = 32;
pub const DEFAULT_MEMORY_RETRIEVAL_MAX_DEPTH: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRetrievalMode {
    CommunityRouted,
    GlobalExact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRetrievalConfig {
    pub mode: MemoryRetrievalMode,
    pub community_limit: usize,
    pub subcentroids_per_community: usize,
    pub seed_limit: usize,
    pub traversal_budget: usize,
    pub max_depth: usize,
}

impl Default for MemoryRetrievalConfig {
    fn default() -> Self {
        Self {
            mode: MemoryRetrievalMode::CommunityRouted,
            community_limit: DEFAULT_MEMORY_RETRIEVAL_COMMUNITIES,
            subcentroids_per_community: DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
            seed_limit: DEFAULT_MEMORY_RETRIEVAL_SEEDS,
            traversal_budget: DEFAULT_MEMORY_RETRIEVAL_BUDGET,
            max_depth: DEFAULT_MEMORY_RETRIEVAL_MAX_DEPTH,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetrievalPolicyError {
    InvalidConfig,
    InvalidQueryVector,
    EmptyPopulation,
    StaleIndex,
    CorruptVector(&'static str),
}

/// Owner-prepared facts for building the disposable Memory retrieval index.
pub struct MemoryRetrievalSnapshot {
    pub compatibility_profile_id: CompatibilityProfileId,
    pub memory_version: u64,
    pub memory_graph_version: u64,
    pub community_generation: Option<u64>,
    pub vector_bindings: usize,
    pub subcentroids_per_community: usize,
    pub community_snapshot: Option<CommunitySnapshot>,
    pub vectors: HashMap<MemoryId, Vec<f32>>,
    pub searchable: HashSet<MemoryId>,
}

#[derive(Clone, Debug)]
pub struct MemoryRetrievalIndex {
    pub compatibility_profile_id: CompatibilityProfileId,
    pub memory_version: u64,
    pub memory_graph_version: u64,
    pub community_generation: Option<u64>,
    pub vector_bindings: usize,
    pub subcentroids_per_community: usize,
    pub dimensions: usize,
    pub indexed_memories: usize,
    pub routing_vectors: usize,
    vectors: HashMap<MemoryId, Vec<f32>>,
    searchable: HashSet<MemoryId>,
    memberships: HashMap<MemoryId, CommunityId>,
    subcentroids: Vec<CommunitySubcentroid>,
}

impl MemoryRetrievalIndex {
    pub fn build(snapshot: MemoryRetrievalSnapshot) -> Result<Self, RetrievalPolicyError> {
        if snapshot.subcentroids_per_community == 0 {
            return Err(RetrievalPolicyError::InvalidConfig);
        }
        let dimensions = snapshot
            .vectors
            .values()
            .next()
            .map(Vec::len)
            .ok_or(RetrievalPolicyError::EmptyPopulation)?;
        let current_community = snapshot.community_snapshot.filter(|community| {
            community.derived_graph_version == snapshot.memory_graph_version
                && community.algorithm_version == COMMUNITY_ALGORITHM_VERSION
        });
        let memberships = current_community
            .as_ref()
            .map(|community| {
                community
                    .communities
                    .iter()
                    .flat_map(|community| {
                        community
                            .members
                            .iter()
                            .map(move |member| (MemoryId(*member), community.id))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let subcentroids = current_community
            .as_ref()
            .map(|community| {
                build_subcentroids(
                    community,
                    &snapshot.vectors,
                    snapshot.subcentroids_per_community,
                )
            })
            .unwrap_or_default();

        Ok(Self {
            compatibility_profile_id: snapshot.compatibility_profile_id,
            memory_version: snapshot.memory_version,
            memory_graph_version: snapshot.memory_graph_version,
            community_generation: snapshot.community_generation,
            vector_bindings: snapshot.vector_bindings,
            subcentroids_per_community: snapshot.subcentroids_per_community,
            dimensions,
            indexed_memories: snapshot.vectors.len(),
            routing_vectors: subcentroids.len(),
            vectors: snapshot.vectors,
            searchable: snapshot.searchable,
            memberships,
            subcentroids,
        })
    }

    pub fn vector_for(&self, id: MemoryId) -> Option<&[f32]> {
        self.vectors.get(&id).map(Vec::as_slice)
    }

    pub fn vectors(&self) -> impl Iterator<Item = (MemoryId, &[f32])> + '_ {
        self.vectors
            .iter()
            .map(|(id, vector)| (*id, vector.as_slice()))
    }

    pub fn subcentroids(&self) -> &[CommunitySubcentroid] {
        &self.subcentroids
    }

    pub fn searchable_ids(&self) -> &HashSet<MemoryId> {
        &self.searchable
    }

    pub fn memberships(&self) -> &HashMap<MemoryId, CommunityId> {
        &self.memberships
    }

    pub fn prepare_retrieval(
        &self,
        query: &[f32],
        config: MemoryRetrievalConfig,
        current_memory_version: u64,
        current_memory_graph_version: u64,
        current_community_generation: Option<u64>,
        current_vector_bindings: usize,
    ) -> Result<MemoryRetrievalPlan, RetrievalPolicyError> {
        validate_config(config)?;
        validate_query(query)?;
        if self.memory_version != current_memory_version
            || self.memory_graph_version != current_memory_graph_version
            || self.community_generation != current_community_generation
            || self.vector_bindings != current_vector_bindings
            || (config.mode == MemoryRetrievalMode::CommunityRouted
                && self.subcentroids_per_community != config.subcentroids_per_community)
        {
            return Err(RetrievalPolicyError::StaleIndex);
        }
        if query.len() != self.dimensions {
            return Err(RetrievalPolicyError::InvalidQueryVector);
        }

        let mut mode_used = config.mode;
        let mut used_global_fallback = false;
        let mut selected_communities = Vec::new();
        let mut routing_vectors_scored = 0;
        let candidate_ids = match config.mode {
            MemoryRetrievalMode::CommunityRouted if !self.subcentroids.is_empty() => {
                routing_vectors_scored = self.subcentroids.len();
                selected_communities =
                    route_communities(&self.subcentroids, query, config.community_limit);
                if selected_communities.is_empty() {
                    mode_used = MemoryRetrievalMode::GlobalExact;
                    used_global_fallback = true;
                    self.sorted_ids()
                } else {
                    let selected: HashSet<_> = selected_communities.iter().copied().collect();
                    let mut admitted = self
                        .vectors
                        .keys()
                        .copied()
                        .filter(|id| {
                            self.memberships
                                .get(id)
                                .is_none_or(|community| selected.contains(community))
                        })
                        .collect::<Vec<_>>();
                    admitted.sort_by_key(|id| id.0);
                    admitted
                }
            }
            MemoryRetrievalMode::CommunityRouted => {
                mode_used = MemoryRetrievalMode::GlobalExact;
                used_global_fallback = true;
                self.sorted_ids()
            }
            MemoryRetrievalMode::GlobalExact => self.sorted_ids(),
        };
        let mut seeds = exact_memory_scores(query, &self.vectors, &candidate_ids)?;
        seeds.truncate(config.seed_limit.min(seeds.len()));
        Ok(MemoryRetrievalPlan {
            mode_used,
            used_global_fallback,
            selected_communities,
            seeds,
            candidate_ids,
            routing_vectors_scored,
            global_memory_vectors: self.vectors.len(),
        })
    }

    fn sorted_ids(&self) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self.vectors.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        ids
    }
}

#[derive(Debug)]
pub struct MemoryRetrievalPlan {
    pub mode_used: MemoryRetrievalMode,
    pub used_global_fallback: bool,
    pub selected_communities: Vec<CommunityId>,
    pub seeds: Vec<MemoryRetrievalHit>,
    pub candidate_ids: Vec<MemoryId>,
    pub routing_vectors_scored: usize,
    pub global_memory_vectors: usize,
}

pub fn exact_memory_scores(
    query: &[f32],
    vectors: &HashMap<MemoryId, Vec<f32>>,
    candidates: &[MemoryId],
) -> Result<Vec<MemoryRetrievalHit>, RetrievalPolicyError> {
    validate_query(query)?;
    let mut ranked = Vec::with_capacity(candidates.len());
    for id in candidates {
        let vector = vectors.get(id).ok_or(RetrievalPolicyError::CorruptVector(
            "missing candidate vector",
        ))?;
        let score = cosine(query, vector).ok_or(RetrievalPolicyError::InvalidQueryVector)?;
        ranked.push(MemoryRetrievalHit {
            memory_id: *id,
            score,
        });
    }
    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    Ok(ranked)
}

pub fn validate_query(query: &[f32]) -> Result<(), RetrievalPolicyError> {
    if query.is_empty() || query.iter().any(|value| !value.is_finite()) {
        return Err(RetrievalPolicyError::InvalidQueryVector);
    }
    let norm = query
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>();
    if norm == 0.0 {
        return Err(RetrievalPolicyError::InvalidQueryVector);
    }
    Ok(())
}

fn validate_config(config: MemoryRetrievalConfig) -> Result<(), RetrievalPolicyError> {
    if config.community_limit == 0
        || config.subcentroids_per_community == 0
        || config.seed_limit == 0
        || config.traversal_budget == 0
    {
        return Err(RetrievalPolicyError::InvalidConfig);
    }
    Ok(())
}
