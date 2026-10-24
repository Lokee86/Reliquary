use crate::archive::ArchiveReadView;
use crate::lexical_contract::{LexicalHit, lexical_terms, term_counts};
use crate::lexical_index::{LexicalIndex, MemoryLexicalHit};
use crate::memory_store::MemoryReadView;
use crate::{
    FileSearchHit, Fragment, Memory, MemoryError, MemoryId, MemoryRoutingMetadata, SearchError,
};

pub(crate) struct MemoryLexicalReadView<'a> {
    index: &'a mut LexicalIndex,
    memories: MemoryReadView<'a>,
}

impl<'a> MemoryLexicalReadView<'a> {
    pub(crate) fn new(index: &'a mut LexicalIndex, memories: MemoryReadView<'a>) -> Self {
        Self { index, memories }
    }

    pub(crate) fn candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let terms = lexical_terms(query);
        self.index.ensure_memories_current(&self.memories)?;
        Ok(self.index.search_memories(&terms, limit))
    }

    pub(crate) fn complete_candidates(
        &mut self,
        query: &str,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let terms = lexical_terms(query);
        self.index.ensure_memories_current(&self.memories)?;
        Ok(self.index.complete_memory_matches(&terms))
    }

    pub(crate) fn memory(&self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(id)
    }
    pub(crate) fn current_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }
    pub(crate) fn routing_metadata(&self, id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.memories.routing_metadata(id)
    }
    pub(crate) fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }
    pub(crate) fn memory_view(&self) -> &MemoryReadView<'a> {
        &self.memories
    }
}

pub(crate) fn complete_memory_lexical_matches(
    view: &mut MemoryLexicalReadView<'_>,
    query: &str,
) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
    view.complete_candidates(query)
}

pub(crate) fn memory_lexical_candidates(
    view: &mut MemoryLexicalReadView<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
    view.candidates(query, limit)
}

pub(crate) fn lexical_candidates(
    index: &mut LexicalIndex,
    archive: &ArchiveReadView<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<LexicalHit>, SearchError> {
    let terms = lexical_terms(query);
    index.ensure_current(archive)?;
    Ok(index.search(&terms, limit))
}

pub(crate) fn raw_archive_candidates(
    index: &mut LexicalIndex,
    archive: &ArchiveReadView<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<LexicalHit>, SearchError> {
    let terms = lexical_terms(query);
    index.ensure_raw_current(archive)?;
    Ok(index.search_raw(&terms, limit))
}

pub(crate) fn lexical_candidates_from_fragments(
    archive: &ArchiveReadView<'_>,
    query: &str,
    fragments: &[Fragment],
    limit: usize,
) -> Result<Vec<LexicalHit>, SearchError> {
    let terms = lexical_terms(query);
    if terms.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    for (position, fragment) in fragments.iter().cloned().enumerate() {
        let text = archive.fragment_text_for(&fragment)?;
        let score = lexical_score(&text, &terms);
        if score > 0.0 {
            hits.push((LexicalHit { fragment, score }, position));
        }
    }
    hits.sort_by(|left, right| {
        right
            .0
            .score
            .total_cmp(&left.0.score)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.0.fragment.id.0.cmp(&right.0.fragment.id.0))
    });
    hits.truncate(limit);
    Ok(hits.into_iter().map(|(hit, _)| hit).collect())
}

pub(crate) fn search_files(
    index: &mut LexicalIndex,
    archive: &ArchiveReadView<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<FileSearchHit>, SearchError> {
    let terms = lexical_terms(query);
    index.ensure_current(archive)?;
    Ok(index.search_files(&terms, limit))
}

#[cfg(test)]
pub(crate) fn lexical_candidates_parts(
    archive: &ArchiveReadView<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<LexicalHit>, SearchError> {
    let terms = lexical_terms(query);
    if terms.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    for fragment in archive.fragments_from(0) {
        let text = archive.fragment_text(fragment.id)?;
        let score = lexical_score(&text, &terms);
        if score > 0.0 {
            let archive_version = archive.fragment_archive_version(fragment.id).unwrap_or(0);
            hits.push((LexicalHit { fragment, score }, archive_version));
        }
    }
    hits.sort_by(|left, right| {
        right
            .0
            .score
            .total_cmp(&left.0.score)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.0.fragment.id.0.cmp(&right.0.fragment.id.0))
    });
    hits.truncate(limit);
    Ok(hits.into_iter().map(|(hit, _)| hit).collect())
}

pub(crate) fn lexical_score(text: &str, terms: &[String]) -> f64 {
    let words = term_counts(text);
    let mut matched_terms = 0_usize;
    let mut frequency = 0_usize;
    for term in terms {
        if let Some(count) = words.get(term).copied().filter(|count| *count > 0) {
            matched_terms += 1;
            frequency += count.min(4);
        }
    }
    if matched_terms == 0 {
        return 0.0;
    }
    let coverage = matched_terms as f64 / terms.len() as f64;
    let density = (frequency as f64 / (terms.len() * 2) as f64).min(1.0);
    0.85 * coverage + 0.15 * density
}
