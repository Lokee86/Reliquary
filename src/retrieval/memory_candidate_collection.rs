use crate::lexical_search::MemoryLexicalReadView;
use crate::memory_candidate_pool::MemoryCandidatePool;
use crate::memory_retrieval_vectors::{
    exact_memory_scores, load_searchable_memory_vectors, validate_query,
};
use crate::memory_store::MemoryReadView;
use crate::memory_vector_store::MemoryVectorReadView;
use crate::{CompatibilityProfile, CompatibilityProfileId, MemoryRetrievalError};
pub(crate) fn collect_memory_candidate_pool(
    memories: &MemoryReadView<'_>,
    memory_vectors: &MemoryVectorReadView<'_>,
    lexical_index: &mut crate::lexical_index::LexicalIndex,
    profile: Option<CompatibilityProfile>,
    query: &str,
    query_vector: &[f32],
    profile_id: CompatibilityProfileId,
) -> Result<MemoryCandidatePool, MemoryRetrievalError> {
    validate_query(query_vector)?;
    if let Some(profile) = profile.as_ref() {
        if query_vector.len() != profile.dimensions as usize {
            return Err(MemoryRetrievalError::InvalidQueryVector);
        }
    }
    let lexical = {
        let mut view = MemoryLexicalReadView::new(lexical_index, *memories);
        view.complete_candidates(query)?
    };
    let (_, vectors) = load_searchable_memory_vectors(memories, memory_vectors, profile_id)?;
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
