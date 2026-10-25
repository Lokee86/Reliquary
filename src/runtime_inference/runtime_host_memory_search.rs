pub use crate::runtime_shared::{
    DEFAULT_MEMORY_SEARCH_DEPTH, DEFAULT_MEMORY_SEARCH_RESULTS, MAX_MEMORY_SEARCH_DEPTH,
    MAX_MEMORY_SEARCH_QUERY_BYTES, MAX_MEMORY_SEARCH_RESULTS, MemorySearchItem, MemorySearchLane,
    MemorySearchResult, VisibleMemorySearchOwner, VisibleMemorySearchResult,
};

use crate::memory_search::{MemoryLexicalSearchHit, validate};
use crate::{Cva, Phylactery, SearchError};

pub(crate) fn search_memories_rel(
    cva: &mut Cva,
    query: &str,
    limit: usize,
) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
    validate(query, limit)?;
    let hits = cva.runtime_memory_lexical_candidates(query.trim(), limit)?;
    hits.into_iter()
        .map(|hit| {
            Ok(MemoryLexicalSearchHit {
                memory: cva.runtime_memory(hit.memory_id)?,
                score: hit.score,
            })
        })
        .collect()
}

pub(crate) fn search_memories_phy(
    phy: &mut Phylactery,
    query: &str,
    limit: usize,
) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
    validate(query, limit)?;
    let hits = phy.runtime_memory_lexical_candidates(query.trim(), limit)?;
    hits.into_iter()
        .map(|hit| {
            Ok(MemoryLexicalSearchHit {
                memory: phy.memory(hit.memory_id)?,
                score: hit.score,
            })
        })
        .collect()
}
