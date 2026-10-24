//! Test fixture binding for the canonical owner-read candidate collector.
#[cfg(test)]
impl crate::Cva {
    pub(crate) fn collect_memory_candidate_pool(
        &mut self,
        query: &str,
        vector: &[f32],
        profile: crate::CompatibilityProfileId,
    ) -> Result<crate::memory_candidate_pool::MemoryCandidatePool, crate::MemoryRetrievalError>
    {
        crate::memory_candidate_collection::collect_memory_candidate_pool(
            &crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
            &crate::memory_vector_store::MemoryVectorReadView::new(
                &self.memory_vectors,
                &self.packed_vectors,
                &self.container,
            ),
            &mut self.lexical_index,
            self.compatibility_profiles.get_active(profile).cloned(),
            query,
            vector,
            profile,
        )
    }
}
