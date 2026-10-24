use crate::lexical_contract::LexicalHit;
use crate::lexical_index::MemoryLexicalHit;
use crate::lexical_search;
use crate::{Cva, FileSearchHit, Fragment, MemoryError, Phylactery, SearchError};

impl Cva {
    pub(crate) fn complete_memory_lexical_matches(
        &mut self,
        query: &str,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        lexical_search::complete_memory_lexical_matches(
            &mut self.lexical_index,
            &self.memories,
            &mut self.container,
            query,
        )
    }

    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        lexical_search::memory_lexical_candidates(
            &mut self.lexical_index,
            &self.memories,
            &mut self.container,
            query,
            limit,
        )
    }

    pub(crate) fn lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        lexical_search::lexical_candidates(
            &mut self.lexical_index,
            &self.archive,
            &mut self.container,
            query,
            limit,
        )
    }

    pub(crate) fn raw_archive_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        lexical_search::raw_archive_candidates(
            &mut self.lexical_index,
            &self.archive,
            &mut self.container,
            query,
            limit,
        )
    }

    pub(crate) fn lexical_candidates_from_fragments(
        &mut self,
        query: &str,
        fragments: &[Fragment],
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        lexical_search::lexical_candidates_from_fragments(
            &self.archive,
            &mut self.container,
            query,
            fragments,
            limit,
        )
    }

    pub fn search_files(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchHit>, SearchError> {
        lexical_search::search_files(
            &mut self.lexical_index,
            &self.archive,
            &mut self.container,
            query,
            limit,
        )
    }
}

impl Phylactery {
    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        lexical_search::memory_lexical_candidates(
            &mut self.lexical_index,
            &self.memories,
            &mut self.container,
            query,
            limit,
        )
    }
}
