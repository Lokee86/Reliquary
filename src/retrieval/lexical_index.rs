use crate::{Archive, ArchiveError};
use reliquary_container::Container;
use reliquary_retrieval::LexicalIndex as RetrievalLexicalIndex;
pub(crate) use reliquary_retrieval::MemoryLexicalHit;

#[path = "lexical_index/memory.rs"]
mod memory;
#[path = "lexical_index/raw.rs"]
mod raw;

#[derive(Default)]
pub(crate) struct LexicalIndex {
    engine: RetrievalLexicalIndex,
}

impl LexicalIndex {
    pub(crate) fn ensure_current(
        &mut self,
        archive: &Archive,
        container: &mut Container,
    ) -> Result<(), ArchiveError> {
        for fragment in archive
            .source_fragments()
            .skip(self.engine.fragment_count())
            .cloned()
            .collect::<Vec<_>>()
        {
            let version = archive
                .source_fragment_archive_version(fragment.id)
                .unwrap_or_default();
            let text = archive.fragment_text(container, fragment.id)?;
            self.engine.add_fragment(fragment, version, &text);
        }
        for file in archive
            .source_files()
            .skip(self.engine.file_count())
            .cloned()
            .collect::<Vec<_>>()
        {
            self.engine.add_file(file);
        }
        Ok(())
    }
    pub(crate) fn search(
        &self,
        terms: &[String],
        limit: usize,
    ) -> Vec<crate::lexical_search::LexicalHit> {
        self.engine.search(terms, limit)
    }
    pub(crate) fn search_files(&self, terms: &[String], limit: usize) -> Vec<crate::FileSearchHit> {
        self.engine.search_files(terms, limit)
    }
}
