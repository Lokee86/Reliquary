//! Supported public retrieval binding over canonical runtime operations.
use crate::{
    CompatibilityProfileId, MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalIndex,
    MemoryRetrievalResult, Phylactery,
};
impl Phylactery {
    pub fn build_memory_retrieval_index(
        &mut self,
        profile: CompatibilityProfileId,
        subcentroids: usize,
    ) -> Result<MemoryRetrievalIndex, MemoryRetrievalError> {
        self.native_build_memory_retrieval_index(profile, subcentroids)
    }
    pub fn retrieve_memories_with_index(
        &self,
        index: &MemoryRetrievalIndex,
        query: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
        self.native_retrieve_memories_with_index(index, query, config)
    }
    pub fn retrieve_memories(
        &mut self,
        profile: CompatibilityProfileId,
        query: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
        self.native_retrieve_memories(profile, query, config)
    }
}
