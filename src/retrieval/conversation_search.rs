use crate::archive_vector_store::ArchiveVectorStore;
use crate::fragmenter::path_fragment_windows;
use crate::lexical_index::LexicalIndex;
use crate::packed_vector_store::PackedVectorStore;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{
    Archive, CompatibilityProfileId, Container, ConversationSearchHit, FragmentConfig,
    RetrievalConfig, SearchError,
};

pub(crate) fn branch_candidate_limit(limit: usize) -> usize {
    crate::DEFAULT_SEARCH_CANDIDATE_LIMIT
        .max(limit)
        .min(crate::MAX_SEMANTIC_SEARCH_LIMIT)
}

pub(crate) fn search_conversation_branch_parts(
    archive: &Archive,
    container: &mut Container,
    lexical_index: &mut LexicalIndex,
    vector_generations: &mut VectorGenerationStore,
    archive_vectors: &mut ArchiveVectorStore,
    packed_vectors: &mut PackedVectorStore,
    compatibility_profile_id: Option<CompatibilityProfileId>,
    query_vector: Option<&[f32]>,
    conversation_id: &str,
    leaf_node_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<ConversationSearchHit>, SearchError> {
    if !(1..=crate::MAX_SEMANTIC_SEARCH_LIMIT).contains(&limit) {
        return Err(SearchError::InvalidConfig);
    }
    archive.require_current_conversation_id(conversation_id)?;
    let nodes = archive.branch_nodes(conversation_id, leaf_node_id)?;
    let fragments = path_fragment_windows(&nodes, FragmentConfig::default(), true)?;
    let candidate_limit = branch_candidate_limit(limit);
    let weights = crate::search::validate_search(
        query,
        RetrievalConfig {
            candidate_limit,
            result_limit: limit,
            ..RetrievalConfig::default()
        },
    )?;
    let lexical = crate::lexical_search::lexical_candidates_from_fragments(
        archive,
        container,
        query,
        &fragments,
        candidate_limit,
    )?;
    let semantic = match (compatibility_profile_id, query_vector) {
        (Some(profile), Some(vector))
            if vector_generations.current_generation(profile).is_some() =>
        {
            let allowed = fragments.iter().map(|fragment| fragment.id).collect();
            crate::semantic_search::semantic_search_vector(
                vector_generations,
                archive_vectors,
                packed_vectors,
                container,
                archive,
                profile,
                vector,
                candidate_limit,
                Some(&allowed),
            )?
        }
        _ => Vec::new(),
    };
    let candidates = crate::search_rank::rank_search_candidates(
        archive,
        lexical,
        semantic,
        RetrievalConfig {
            candidate_limit,
            result_limit: limit,
            ..RetrievalConfig::default()
        },
        weights,
    )?;
    candidates
        .into_iter()
        .map(|candidate| {
            let text = archive.fragment_text_for(container, &candidate.fragment)?;
            Ok(ConversationSearchHit {
                fragment: candidate.fragment,
                text,
                score: candidate.combined_score,
            })
        })
        .collect()
}
