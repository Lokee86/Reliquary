use crate::graph_owner::GraphOwner;
use crate::memory_retrieval_traversal::traverse;
use crate::{
    MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalIndex, MemoryRetrievalResult,
};

pub(crate) fn retrieve_memories(
    graph: &GraphOwner,
    current_memory_version: u64,
    current_community_generation: Option<u64>,
    current_vector_bindings: usize,
    index: &MemoryRetrievalIndex,
    query: &[f32],
    config: MemoryRetrievalConfig,
) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
    let plan = index.prepare_retrieval(
        query,
        config,
        current_memory_version,
        graph.memory_graph_version(),
        current_community_generation,
        current_vector_bindings,
    )?;
    let seed_ids = plan
        .seeds
        .iter()
        .map(|hit| hit.memory_id)
        .collect::<Vec<_>>();
    let memories = traverse(
        &graph.active_relations(),
        index.memberships(),
        index.searchable_ids(),
        &seed_ids,
        config.traversal_budget,
        config.max_depth,
        plan.mode_used == crate::MemoryRetrievalMode::CommunityRouted,
    );
    Ok(MemoryRetrievalResult {
        mode_used: plan.mode_used,
        used_global_fallback: plan.used_global_fallback,
        selected_communities: plan.selected_communities,
        seeds: plan.seeds,
        memories,
        routing_vectors_scored: plan.routing_vectors_scored,
        memory_vectors_scored: plan.candidate_ids.len(),
        global_memory_vectors: plan.global_memory_vectors,
    })
}
