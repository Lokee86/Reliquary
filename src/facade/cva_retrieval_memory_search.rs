use crate::memory_search::{MemoryLexicalSearchHit, validate};
use crate::{Cva, Phylactery, SearchError};

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
