use crate::lexical_search::MemoryLexicalReadView;
use crate::memory_candidate_pool::MemoryCandidatePool;
use crate::memory_retrieval_vectors::{
    exact_memory_scores, load_searchable_memory_vectors, validate_query,
};
use crate::memory_store::MemoryReadView;
use crate::{CompatibilityProfileId, Cva, MemoryRetrievalError};

impl Cva {
    /// REL-local complete eligibility union. Scores are evidence, never presentation policy.
    pub(crate) fn collect_memory_candidate_pool(
        &mut self,
        query: &str,
        query_vector: &[f32],
        profile_id: CompatibilityProfileId,
    ) -> Result<MemoryCandidatePool, MemoryRetrievalError> {
        validate_query(query_vector)?;
        if let Some(profile) = self.compatibility_profiles.get_active(profile_id) {
            if query_vector.len() != profile.dimensions as usize {
                return Err(MemoryRetrievalError::InvalidQueryVector);
            }
        }
        let lexical = {
            let memory_read = MemoryReadView::new(&self.memories, &self.container);
            let mut view = MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
            view.complete_candidates(query)?
        };
        let (_, vectors) = load_searchable_memory_vectors(
            &mut self.container,
            &self.memories,
            &self.memory_vectors,
            &self.packed_vectors,
            profile_id,
        )?;
        let mut ids = vectors.keys().copied().collect::<Vec<_>>();
        ids.sort();
        let semantic = exact_memory_scores(query_vector, &vectors, &ids)?;
        Ok(
            crate::memory_candidate_pool::merge_memory_candidate_evidence(
                profile_id,
                lexical.into_iter().map(|hit| (hit.memory_id, hit.score)),
                semantic.into_iter().map(|hit| (hit.memory_id, hit.score)),
            ),
        )
    }
}
