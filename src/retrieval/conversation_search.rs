use crate::archive::ArchiveReadView;
use crate::fragmenter::path_fragment_windows;
use crate::lexical_index::LexicalIndex;
use crate::vector_generation_store::VectorReadView;
use crate::{
    CompatibilityProfileId, ConversationSearchHit, FragmentConfig, RetrievalConfig, SearchError,
};

pub(crate) fn branch_candidate_limit(limit: usize) -> usize {
    crate::DEFAULT_SEARCH_CANDIDATE_LIMIT
        .max(limit)
        .min(crate::MAX_SEMANTIC_SEARCH_LIMIT)
}

pub(crate) fn search_conversation_branch_parts(
    archive: &ArchiveReadView<'_>,
    lexical_index: &mut LexicalIndex,
    vectors: &VectorReadView<'_>,
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
        query,
        &fragments,
        candidate_limit,
    )?;
    let semantic = match (compatibility_profile_id, query_vector) {
        (Some(profile), Some(vector)) if vectors.current_generation(profile).is_some() => {
            let allowed = fragments.iter().map(|fragment| fragment.id).collect();
            crate::semantic_search::semantic_search_vector(
                vectors,
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
            let text = archive.fragment_text_for(&candidate.fragment)?;
            Ok(ConversationSearchHit {
                fragment: candidate.fragment,
                text,
                score: candidate.combined_score,
            })
        })
        .collect()
}
