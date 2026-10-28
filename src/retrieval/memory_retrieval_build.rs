use crate::graph_owner::GraphOwner;
use crate::memory_retrieval_vectors::load_searchable_memory_vectors;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::{
    CommunitySnapshot, CompatibilityProfileId, MemoryRetrievalError, MemoryRetrievalIndex,
};
use reliquary_container::Container;
use reliquary_retrieval::memory_retrieval::MemoryRetrievalSnapshot;
use reliquary_vector_store::PackedVectorStore;

pub(crate) fn build_memory_retrieval_index(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &GraphOwner,
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
    MemoryRetrievalIndex::build(MemoryRetrievalSnapshot {
        compatibility_profile_id: profile_id,
        memory_version: memories.memory_version(),
        memory_graph_version: graph.memory_graph_version(),
        community_generation: snapshot.map(|snapshot| snapshot.generation),
        vector_bindings: memory_vectors.binding_count(profile_id),
        subcentroids_per_community,
        community_snapshot: snapshot.cloned().map(Into::into),
        vectors,
        searchable,
    })
    .map_err(Into::into)
}
