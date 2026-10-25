use crate::lexical_contract::LexicalHit;
use crate::lexical_index::MemoryLexicalHit;
use crate::{Cva, FileSearchHit, Fragment, MemoryError, Phylactery, SearchError};

impl Cva {
    pub(crate) fn complete_memory_lexical_matches(
        &mut self,
        query: &str,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        self.runtime_complete_memory_lexical_matches(query)
    }

    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        self.runtime_memory_lexical_candidates(query, limit)
    }

    pub(crate) fn lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        self.runtime_lexical_candidates(query, limit)
    }

    pub(crate) fn raw_archive_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        self.runtime_raw_archive_candidates(query, limit)
    }

    pub(crate) fn lexical_candidates_from_fragments(
        &mut self,
        query: &str,
        fragments: &[Fragment],
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        self.runtime_lexical_candidates_from_fragments(query, fragments, limit)
    }

    pub fn search_files(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchHit>, SearchError> {
        self.runtime_search_files(query, limit)
    }
}

impl Phylactery {
    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        self.runtime_memory_lexical_candidates(query, limit)
    }
}
