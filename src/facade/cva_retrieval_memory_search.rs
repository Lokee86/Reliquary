use crate::{Cva, MemoryLexicalSearchHit, Phylactery, SearchError};

impl Cva {
    /// Return lexical candidates without access reinforcement. Even hydrated hits
    /// are not proof that their content was accepted into an external context.
    pub fn search_memories(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
        crate::runtime_host::memory_search::search_memories_rel(self, query, limit)
    }
}

impl Phylactery {
    /// Return lexical candidates without access reinforcement.
    pub fn search_memories(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalSearchHit>, SearchError> {
        crate::runtime_host::memory_search::search_memories_phy(self, query, limit)
    }
}
