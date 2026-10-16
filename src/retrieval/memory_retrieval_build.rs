use crate::graph_store::GraphStore;
use crate::memory_retrieval_index::build_subcentroids;
use crate::memory_retrieval_vectors::load_searchable_memory_vectors;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, CommunityId, CommunitySnapshot, CompatibilityProfileId, Container,
    MemoryId, MemoryRetrievalError, MemoryRetrievalIndex,
};
use std::collections::HashMap;

pub(crate) fn build_memory_retrieval_index(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &GraphStore,
    snapshot: Option<&CommunitySnapshot>,
    memory_vectors: &MemoryVectorStore,
    packed_vectors: &PackedVectorStore,
    profile_id: CompatibilityProfileId,
    subcentroids_per_community: usize,
) -> Result<MemoryRetrievalIndex, MemoryRetrievalError> {
    if subcentroids_per_community == 0 {
        return Err(MemoryRetrievalError::InvalidConfig);
    }
    let (searchable, vectors) = load_searchable_memory_vectors(
        container,
        memories,
        memory_vectors,
        packed_vectors,
        profile_id,
    )?;
    let dimensions = vectors
        .values()
        .next()
        .map(Vec::len)
        .ok_or(MemoryRetrievalError::EmptyPopulation)?;

    let current_snapshot = snapshot.filter(|snapshot| {
        snapshot.derived_graph_version == graph.memory_graph_version()
            && snapshot.algorithm_version == COMMUNITY_ALGORITHM_VERSION
    });
    let memberships = current_snapshot.map(membership_map).unwrap_or_default();
    let subcentroids = current_snapshot
        .map(|snapshot| build_subcentroids(snapshot, &vectors, subcentroids_per_community))
        .unwrap_or_default();

    Ok(MemoryRetrievalIndex {
        compatibility_profile_id: profile_id,
        memory_version: memories.memory_version(),
        memory_graph_version: graph.memory_graph_version(),
        community_generation: snapshot.map(|snapshot| snapshot.generation),
        vector_bindings: memory_vectors.binding_count(profile_id),
        subcentroids_per_community,
        dimensions,
        indexed_memories: vectors.len(),
        routing_vectors: subcentroids.len(),
        vectors,
        searchable,
        memberships,
        subcentroids,
    })
}

fn membership_map(snapshot: &CommunitySnapshot) -> HashMap<MemoryId, CommunityId> {
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
