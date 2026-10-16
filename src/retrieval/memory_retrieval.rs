use crate::graph_store::GraphStore;
use crate::memory_retrieval_index::route_communities;
use crate::memory_retrieval_traversal::traverse;
use crate::memory_retrieval_vectors::{exact_memory_scores, validate_query};
use crate::{
    MemoryId, MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalHit,
    MemoryRetrievalIndex, MemoryRetrievalMode, MemoryRetrievalResult,
};
use std::collections::HashSet;

pub(crate) fn retrieve_memories(
    graph: &GraphStore,
    current_memory_version: u64,
    current_community_generation: Option<u64>,
    current_vector_bindings: usize,
    index: &MemoryRetrievalIndex,
    query: &[f32],
    config: MemoryRetrievalConfig,
) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
    validate_config(config)?;
    validate_query(query)?;
    validate_index(
        graph,
        current_memory_version,
        current_community_generation,
        current_vector_bindings,
        index,
        config,
    )?;
    if query.len() != index.dimensions {
        return Err(MemoryRetrievalError::InvalidQueryVector);
    }

    let mut mode_used = config.mode;
    let mut used_global_fallback = false;
    let mut selected_communities = Vec::new();
    let mut routing_vectors_scored = 0_usize;

    let candidate_ids = match config.mode {
        MemoryRetrievalMode::CommunityRouted if !index.subcentroids.is_empty() => {
            routing_vectors_scored = index.subcentroids.len();
            selected_communities =
                route_communities(&index.subcentroids, query, config.community_limit);
            if selected_communities.is_empty() {
                mode_used = MemoryRetrievalMode::GlobalExact;
                used_global_fallback = true;
                sorted_ids(index)
            } else {
                let selected: HashSet<_> = selected_communities.iter().copied().collect();
                let mut admitted: Vec<_> = index
                    .vectors
                    .keys()
                    .copied()
                    .filter(|id| {
                        index
                            .memberships
                            .get(id)
                            .is_none_or(|community| selected.contains(community))
                    })
                    .collect();
                admitted.sort_by_key(|id| id.0);
                admitted
            }
        }
        MemoryRetrievalMode::CommunityRouted => {
            mode_used = MemoryRetrievalMode::GlobalExact;
            used_global_fallback = true;
            sorted_ids(index)
        }
        MemoryRetrievalMode::GlobalExact => sorted_ids(index),
    };

    let seeds = exact_search(query, index, &candidate_ids, config.seed_limit)?;
    let seed_ids: Vec<_> = seeds.iter().map(|hit| hit.memory_id).collect();
    let ordered = traverse(
        &graph.active_relations(),
        &index.memberships,
        &index.searchable,
        &seed_ids,
        config.traversal_budget,
        config.max_depth,
        mode_used == MemoryRetrievalMode::CommunityRouted,
    );

    Ok(MemoryRetrievalResult {
        mode_used,
        used_global_fallback,
        selected_communities,
        seeds,
        memories: ordered,
        routing_vectors_scored,
        memory_vectors_scored: candidate_ids.len(),
        global_memory_vectors: index.vectors.len(),
    })
}

fn exact_search(
    query: &[f32],
    index: &MemoryRetrievalIndex,
    candidates: &[MemoryId],
    limit: usize,
) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError> {
    let mut ranked = exact_memory_scores(query, &index.vectors, candidates)?;
    ranked.truncate(limit.min(ranked.len()));
    Ok(ranked)
}

fn sorted_ids(index: &MemoryRetrievalIndex) -> Vec<MemoryId> {
    let mut ids: Vec<_> = index.vectors.keys().copied().collect();
    ids.sort_by_key(|id| id.0);
    ids
}

fn validate_config(config: MemoryRetrievalConfig) -> Result<(), MemoryRetrievalError> {
    if config.community_limit == 0
        || config.subcentroids_per_community == 0
        || config.seed_limit == 0
        || config.traversal_budget == 0
    {
        return Err(MemoryRetrievalError::InvalidConfig);
    }
    Ok(())
}

fn validate_index(
    graph: &GraphStore,
    current_memory_version: u64,
    current_community_generation: Option<u64>,
    current_vector_bindings: usize,
    index: &MemoryRetrievalIndex,
    config: MemoryRetrievalConfig,
) -> Result<(), MemoryRetrievalError> {
    if index.memory_version != current_memory_version
        || index.memory_graph_version != graph.memory_graph_version()
        || index.community_generation != current_community_generation
        || index.vector_bindings != current_vector_bindings
        || (config.mode == MemoryRetrievalMode::CommunityRouted
            && index.subcentroids_per_community != config.subcentroids_per_community)
    {
        return Err(MemoryRetrievalError::StaleIndex);
    }
    Ok(())
}
