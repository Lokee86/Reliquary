use crate::lexical_contract::LexicalHit;
use crate::lexical_index::MemoryLexicalHit;
use crate::lexical_search;
use crate::lexical_search::MemoryLexicalReadView;
use crate::memory_store::MemoryReadView;
use crate::{Cva, FileSearchHit, Fragment, MemoryError, Phylactery, SearchError};

impl Cva {
    pub(crate) fn complete_memory_lexical_matches(
        &mut self,
        query: &str,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let memory_read = MemoryReadView::new(&self.memories, &self.container);
        let mut view = MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        lexical_search::complete_memory_lexical_matches(&mut view, query)
    }

    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let memory_read = MemoryReadView::new(&self.memories, &self.container);
        let mut view = MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        lexical_search::memory_lexical_candidates(&mut view, query, limit)
    }

    pub(crate) fn lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        let archive_read = crate::archive::ArchiveReadView::new(&self.archive, &self.container);
        lexical_search::lexical_candidates(&mut self.lexical_index, &archive_read, query, limit)
    }

    pub(crate) fn raw_archive_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        let archive_read = crate::archive::ArchiveReadView::new(&self.archive, &self.container);
        lexical_search::raw_archive_candidates(&mut self.lexical_index, &archive_read, query, limit)
    }

    pub(crate) fn lexical_candidates_from_fragments(
        &mut self,
        query: &str,
        fragments: &[Fragment],
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        let archive_read = crate::archive::ArchiveReadView::new(&self.archive, &self.container);
        lexical_search::lexical_candidates_from_fragments(&archive_read, query, fragments, limit)
    }

    pub fn search_files(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchHit>, SearchError> {
        let archive_read = crate::archive::ArchiveReadView::new(&self.archive, &self.container);
        lexical_search::search_files(&mut self.lexical_index, &archive_read, query, limit)
    }
}

impl Phylactery {
    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let memory_read = MemoryReadView::new(&self.memories, &self.container);
        let mut view = MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        lexical_search::memory_lexical_candidates(&mut view, query, limit)
    }
}
