use crate::memory_retrieval::retrieve_memories;
use crate::memory_retrieval_build::build_memory_retrieval_index;
use crate::{
    CompatibilityProfileId, MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalIndex,
    MemoryRetrievalResult, Phylactery,
};

impl Phylactery {
    pub fn build_memory_retrieval_index(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        subcentroids_per_community: usize,
    ) -> Result<MemoryRetrievalIndex, MemoryRetrievalError> {
        build_memory_retrieval_index(
            &crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
            &self.graph,
            self.communities.latest(),
            &crate::memory_vector_store::MemoryVectorReadView::new(
                &self.memory_vectors,
                &self.packed_vectors,
                &self.container,
            ),
            compatibility_profile_id,
            subcentroids_per_community,
        )
    }

    pub fn retrieve_memories_with_index(
        &self,
        index: &MemoryRetrievalIndex,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
        retrieve_memories(
            &self.graph,
            self.memories.memory_version(),
            self.communities
                .latest()
                .map(|snapshot| snapshot.generation),
            self.memory_vectors
                .binding_count(index.compatibility_profile_id),
            index,
            query_vector,
            config,
        )
    }

    pub fn retrieve_memories(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
        let index = self.build_memory_retrieval_index(
            compatibility_profile_id,
            config.subcentroids_per_community,
        )?;
        self.retrieve_memories_with_index(&index, query_vector, config)
    }
}
