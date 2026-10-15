use crate::archive_search::{MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS};
use crate::{Cva, Memory, Phylactery, SearchError};

pub const MAX_MEMORY_LEXICAL_SEARCH_QUERY_BYTES: usize = MAX_ARCHIVE_SEARCH_QUERY_BYTES;
pub const MAX_MEMORY_LEXICAL_SEARCH_RESULTS: usize = MAX_ARCHIVE_SEARCH_RESULTS;

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryLexicalSearchHit {
    pub memory: Memory,
    pub score: f64,
}

impl Cva {
    /// Return lexical candidates without access reinforcement. Even hydrated hits
    /// are not proof that their content was accepted into an external context.
    pub fn search_memories(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
        validate(query, limit)?;
        let hits = self.memory_lexical_candidates(query.trim(), limit)?;
        hits.into_iter()
            .map(|hit| {
                Ok(MemoryLexicalSearchHit {
                    memory: self.memory(hit.memory_id)?,
                    score: hit.score,
                })
            })
            .collect()
    }
}

impl Phylactery {
    /// Return lexical candidates without access reinforcement.
    pub fn search_memories(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
        validate(query, limit)?;
        let hits = self.memory_lexical_candidates(query.trim(), limit)?;
        hits.into_iter()
            .map(|hit| {
                Ok(MemoryLexicalSearchHit {
                    memory: self.memory(hit.memory_id)?,
                    score: hit.score,
                })
            })
            .collect()
    }
}

fn validate(query: &str, limit: usize) -> Result<(), SearchError> {
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_MEMORY_LEXICAL_SEARCH_QUERY_BYTES {
        return Err(SearchError::EmptyQuery);
    }
    if !(1..=MAX_MEMORY_LEXICAL_SEARCH_RESULTS).contains(&limit) {
        return Err(SearchError::InvalidConfig);
    }
    Ok(())
}
