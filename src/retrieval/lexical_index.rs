use crate::lexical_contract::{LexicalHit, term_counts};

#[path = "lexical_index/memory.rs"]
mod memory;
#[path = "lexical_index/raw.rs"]
mod raw;
use crate::archive::ArchiveReadView;
use crate::{ArchiveError, FileSearchHit, Fragment, MemoryError, MemoryId, StoredFile};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct LexicalIndex {
    fragments: Vec<IndexedFragment>,
    postings: HashMap<String, Vec<Posting>>,
    raw_turns: Vec<IndexedRawTurn>,
    raw_postings: HashMap<String, Vec<Posting>>,
    files: Vec<StoredFile>,
    file_postings: HashMap<String, Vec<FilePosting>>,
    memory_version: u64,
    memories: Vec<MemoryId>,
    memory_active: Vec<bool>,
    memory_slots: HashMap<MemoryId, usize>,
    memory_postings: HashMap<String, Vec<MemoryPosting>>,
}

#[derive(Clone)]
struct IndexedFragment {
    fragment: Fragment,
    archive_version: u64,
}

#[derive(Clone)]
struct IndexedRawTurn {
    fragment: Fragment,
    timestamp_ns: i64,
}

#[derive(Clone, Copy)]
struct Posting {
    fragment_slot: usize,
    frequency: u8,
}

#[derive(Clone, Copy)]
struct FilePosting {
    file_slot: usize,
    frequency: u8,
}

#[derive(Clone, Copy)]
struct MemoryPosting {
    memory_slot: usize,
    frequency: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MemoryLexicalHit {
    pub(crate) memory_id: MemoryId,
    pub(crate) score: f64,
}

impl LexicalIndex {
    pub(crate) fn ensure_current(
        &mut self,
        archive: &ArchiveReadView<'_>,
    ) -> Result<(), ArchiveError> {
        for fragment in archive.fragments_from(self.fragments.len()) {
            let text = archive.fragment_text_for(&fragment)?;
            let counts = term_counts(&text);
            let slot = self.fragments.len();
            for (term, count) in counts {
                self.postings.entry(term).or_default().push(Posting {
                    fragment_slot: slot,
                    frequency: bounded_frequency(count),
                });
            }
            self.fragments.push(IndexedFragment {
                fragment: fragment.clone(),
                archive_version: archive.fragment_archive_version(fragment.id).unwrap_or(0),
            });
        }
        for file in archive.files_from(self.files.len()) {
            let counts = term_counts(&file.filename);
            let slot = self.files.len();
            for (term, count) in counts {
                self.file_postings
                    .entry(term)
                    .or_default()
                    .push(FilePosting {
                        file_slot: slot,
                        frequency: bounded_frequency(count),
                    });
            }
            self.files.push(file.clone());
        }
        Ok(())
    }

    pub(crate) fn search(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores: HashMap<usize, ScoreParts> = HashMap::new();
        for term in terms {
            let Some(postings) = self.postings.get(term) else {
                continue;
            };
            for posting in postings {
                add_score(&mut scores, posting.fragment_slot, posting.frequency);
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| {
                let indexed = &self.fragments[slot];
                (
                    LexicalHit {
                        fragment: indexed.fragment.clone(),
                        score: score(parts, terms.len()),
                    },
                    indexed.archive_version,
                )
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .0
                .score
                .total_cmp(&left.0.score)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.0.fragment.id.0.cmp(&right.0.fragment.id.0))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(hit, _)| hit).collect()
    }

    pub(crate) fn search_files(&self, terms: &[String], limit: usize) -> Vec<FileSearchHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores: HashMap<usize, ScoreParts> = HashMap::new();
        for term in terms {
            let Some(postings) = self.file_postings.get(term) else {
                continue;
            };
            for posting in postings {
                add_score(&mut scores, posting.file_slot, posting.frequency);
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| FileSearchHit {
                file: self.files[slot].clone(),
                score: score(parts, terms.len()),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.file.filename.cmp(&right.file.filename))
                .then_with(|| left.file.id.0.cmp(&right.file.id.0))
        });
        hits.truncate(limit);
        hits
    }
}

#[derive(Clone, Copy, Default)]
struct ScoreParts {
    matched_terms: usize,
    frequency: usize,
}

fn bounded_frequency(count: usize) -> u8 {
    u8::try_from(count.min(4)).unwrap_or(4)
}

fn add_score(scores: &mut HashMap<usize, ScoreParts>, slot: usize, frequency: u8) {
    let entry = scores.entry(slot).or_default();
    entry.matched_terms += 1;
    entry.frequency += usize::from(frequency);
}

fn score(parts: ScoreParts, term_count: usize) -> f64 {
    let coverage = parts.matched_terms as f64 / term_count as f64;
    let density = (parts.frequency as f64 / (term_count * 2) as f64).min(1.0);
    0.85 * coverage + 0.15 * density
}
