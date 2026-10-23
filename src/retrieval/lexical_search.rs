use crate::lexical_contract::{LexicalHit, lexical_terms, term_counts};
use crate::lexical_index::MemoryLexicalHit;
#[cfg(test)]
use crate::{Archive, Container};
use crate::{Cva, FileSearchHit, Fragment, MemoryError, Phylactery, SearchError};

impl Cva {
    pub(crate) fn complete_memory_lexical_matches(
        &mut self,
        query: &str,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_memories_current(&self.memories, &mut self.container)?;
        Ok(self.lexical_index.complete_memory_matches(&terms))
    }

    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_memories_current(&self.memories, &mut self.container)?;
        Ok(self.lexical_index.search_memories(&terms, limit))
    }

    pub(crate) fn lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_current(&self.archive, &mut self.container)?;
        Ok(self.lexical_index.search(&terms, limit))
    }

    pub(crate) fn raw_archive_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LexicalHit>, SearchError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_raw_current(&self.archive, &mut self.container)?;
        Ok(self.lexical_index.search_raw(&terms, limit))
    }

    pub(crate) fn lexical_candidates_from_fragments(
        &mut self,
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
            let text = self
                .archive
                .fragment_text_for(&mut self.container, &fragment)?;
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

    pub fn search_files(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FileSearchHit>, SearchError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_current(&self.archive, &mut self.container)?;
        Ok(self.lexical_index.search_files(&terms, limit))
    }
}

impl Phylactery {
    pub(crate) fn memory_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryLexicalHit>, MemoryError> {
        let terms = lexical_terms(query);
        self.lexical_index
            .ensure_memories_current(&self.memories, &mut self.container)?;
        Ok(self.lexical_index.search_memories(&terms, limit))
    }
}

#[cfg(test)]
pub(crate) fn lexical_candidates_parts(
    archive: &Archive,
    container: &mut Container,
    query: &str,
    limit: usize,
) -> Result<Vec<LexicalHit>, SearchError> {
    let terms = lexical_terms(query);
    if terms.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    for fragment in archive.fragments() {
        let text = archive.fragment_text(container, fragment.id)?;
        let score = lexical_score(&text, &terms);
        if score > 0.0 {
            let archive_version = archive.fragments.archive_version(fragment.id).unwrap_or(0);
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
