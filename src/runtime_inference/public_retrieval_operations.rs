//! Canonical interactive/offline retrieval invocation over one owner's facts.
use crate::{
    CompatibilityProfileId, MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalIndex,
    MemoryRetrievalResult,
};

macro_rules! retrieval_operations {
    ($runtime:ty) => {
        impl $runtime {
            pub(crate) fn native_build_memory_retrieval_index(
                &mut self,
                profile: CompatibilityProfileId,
                subcentroids: usize,
            ) -> Result<MemoryRetrievalIndex, MemoryRetrievalError> {
                let facts = self.owner.memory_retrieval_facts(profile);
                crate::memory_retrieval_build::build_memory_retrieval_index(
                    &facts.memories,
                    facts.graph,
                    facts.communities,
                    &facts.vectors,
                    profile,
                    subcentroids,
                )
            }
            pub(crate) fn native_retrieve_memories_with_index(
                &self,
                index: &MemoryRetrievalIndex,
                query: &[f32],
                config: MemoryRetrievalConfig,
            ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
                let facts = self
                    .owner
                    .memory_retrieval_facts(index.compatibility_profile_id);
                crate::memory_retrieval::retrieve_memories(
                    facts.graph,
                    facts.memory_version,
                    facts.communities.map(|s| s.generation),
                    facts.vector_bindings,
                    index,
                    query,
                    config,
                )
            }
            pub(crate) fn native_retrieve_memories(
                &mut self,
                profile: CompatibilityProfileId,
                query: &[f32],
                config: MemoryRetrievalConfig,
            ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
                let index = self.native_build_memory_retrieval_index(
                    profile,
                    config.subcentroids_per_community,
                )?;
                self.native_retrieve_memories_with_index(&index, query, config)
            }
        }
    };
}
retrieval_operations!(crate::RelExecutionOwner);
retrieval_operations!(crate::phy_execution_owner::PhyExecutionOwner);
