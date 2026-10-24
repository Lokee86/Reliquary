use crate::archive_search::{MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS};
use crate::{Memory, SearchError};

pub const MAX_MEMORY_LEXICAL_SEARCH_QUERY_BYTES: usize = MAX_ARCHIVE_SEARCH_QUERY_BYTES;
pub const MAX_MEMORY_LEXICAL_SEARCH_RESULTS: usize = MAX_ARCHIVE_SEARCH_RESULTS;

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryLexicalSearchHit {
    pub memory: Memory,
    pub score: f64,
}

pub(crate) fn validate_memory_lexical_query(query: &str) -> Result<&str, SearchError> {
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_MEMORY_LEXICAL_SEARCH_QUERY_BYTES {
        return Err(SearchError::EmptyQuery);
    }
    Ok(query)
}

pub(crate) fn validate(query: &str, limit: usize) -> Result<(), SearchError> {
    validate_memory_lexical_query(query)?;
    if !(1..=MAX_MEMORY_LEXICAL_SEARCH_RESULTS).contains(&limit) {
        return Err(SearchError::InvalidConfig);
    }
    Ok(())
}
