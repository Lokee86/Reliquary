use crate::{RetrievalConfig, SearchError};

#[cfg(test)]
pub(crate) use crate::search_rank::{combine_scores, deduplicate_candidates};

pub(crate) fn validate_search(
    query: &str,
    config: RetrievalConfig,
) -> Result<(f64, f64), SearchError> {
    if query.trim().is_empty() {
        return Err(SearchError::EmptyQuery);
    }
    let Some(weights) = config.normalized_weights() else {
        return Err(SearchError::InvalidConfig);
    };
    if config.candidate_limit > crate::MAX_SEMANTIC_SEARCH_LIMIT {
        return Err(SearchError::InvalidConfig);
    }
    Ok(weights)
}
