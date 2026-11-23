//! Whole-word postings with the exact capped substring frequency used in retrieval.
use aho_corasick::AhoCorasick;
use std::{
    collections::{HashMap, HashSet},
    mem::size_of_val,
    sync::Arc,
};

use crate::ArchiveError;

mod codec;

const FREQUENCY_BITS: u32 = 2;
pub const LEXICAL_FREQUENCY_CAP: usize = 1 << FREQUENCY_BITS;

pub fn lexical_terms(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .filter(|word| seen.insert(word.clone()))
        .collect()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LexicalMatchCounts {
    pub matched_terms: usize,
    pub frequency: usize,
}

/// Exact sizes of the index's retained dictionary, offsets, and postings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FragmentLexicalIndexStats {
    pub terms: usize,
    pub postings: usize,
    pub dictionary_bytes: usize,
    pub offset_bytes: usize,
    pub posting_bytes: usize,
}

/// Inverted index over the readable rows of a captured fragment mapping.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FragmentLexicalIndex {
    rows: usize,
    dictionary: Box<str>,
    offsets: Box<[TermOffset]>,
    postings: Box<[u8]>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TermOffset {
    word: usize,
    posting: usize,
}

#[derive(Default)]
struct PostingList {
    previous_row: usize,
    bytes: Vec<u8>,
}

impl PostingList {
    fn push(&mut self, row: usize, frequency: usize) {
        // Each unsigned varint holds a row delta and frequency minus one.
        let mut value = ((row - self.previous_row) << FREQUENCY_BITS) | (frequency - 1);
        while value >= 128 {
            self.bytes.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        self.bytes.push(value as u8);
        self.previous_row = row;
    }
}

impl FragmentLexicalIndex {
    pub fn build(fragments: &[Option<Vec<Arc<String>>>]) -> Result<Self, ArchiveError> {
        let mut lists = HashMap::<String, PostingList>::new();
        for (row, fragment) in fragments.iter().enumerate() {
            let Some(turns) = fragment else { continue };
            let text = turns
                .iter()
                .map(|turn| turn.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let terms = lexical_terms(&text);
            if terms.is_empty() {
                continue;
            }
            let matcher =
                AhoCorasick::new(&terms).map_err(|_| ArchiveError::EvidenceLimitExceeded)?;
            let lower = text.to_lowercase();
            let mut occurrences = vec![(0, 0); terms.len()];
            for found in matcher.find_overlapping_iter(&lower) {
                let (through, count) = &mut occurrences[found.pattern().as_usize()];
                // Non-overlap is per term, matching str::matches for each pattern.
                if *count < LEXICAL_FREQUENCY_CAP && found.start() >= *through {
                    *through = found.end();
                    *count += 1;
                }
            }
            for (term, (_, count)) in terms.into_iter().zip(occurrences) {
                if count > 0 {
                    lists.entry(term).or_default().push(row, count);
                }
            }
        }

        let mut lists: Vec<_> = lists.into_iter().collect();
        lists.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let mut dictionary = String::new();
        let mut offsets = Vec::with_capacity(lists.len() + 1);
        let mut postings = Vec::new();
        for (term, list) in lists {
            offsets.push(TermOffset {
                word: dictionary.len(),
                posting: postings.len(),
            });
            dictionary.push_str(&term);
            postings.extend(list.bytes);
        }
        offsets.push(TermOffset {
            word: dictionary.len(),
            posting: postings.len(),
        });
        Ok(Self {
            rows: fragments.len(),
            dictionary: dictionary.into_boxed_str(),
            offsets: offsets.into_boxed_slice(),
            postings: postings.into_boxed_slice(),
        })
    }

    pub fn counts(&self, terms: &[String]) -> Vec<LexicalMatchCounts> {
        let mut evidence = vec![LexicalMatchCounts::default(); self.rows];
        for term in terms {
            let mut row = 0;
            let mut value = 0;
            let mut shift = 0;
            for &byte in self.postings_for(term) {
                value |= usize::from(byte & 127) << shift;
                if byte >= 128 {
                    shift += 7;
                    continue;
                }
                row += value >> FREQUENCY_BITS;
                evidence[row].matched_terms += 1;
                evidence[row].frequency += (value & (LEXICAL_FREQUENCY_CAP - 1)) + 1;
                value = 0;
                shift = 0;
            }
        }
        evidence
    }

    pub fn stats(&self) -> FragmentLexicalIndexStats {
        FragmentLexicalIndexStats {
            terms: self.offsets.len() - 1,
            postings: self.postings.iter().filter(|byte| **byte < 128).count(),
            dictionary_bytes: self.dictionary.len(),
            offset_bytes: size_of_val(self.offsets.as_ref()),
            posting_bytes: self.postings.len(),
        }
    }

    fn postings_for(&self, term: &str) -> &[u8] {
        let mut first = 0;
        let mut end = self.offsets.len() - 1;
        while first < end {
            let mid = first + (end - first) / 2;
            let start = &self.offsets[mid];
            let next = &self.offsets[mid + 1];
            match self.dictionary[start.word..next.word].cmp(term) {
                std::cmp::Ordering::Less => first = mid + 1,
                std::cmp::Ordering::Greater => end = mid,
                std::cmp::Ordering::Equal => return &self.postings[start.posting..next.posting],
            }
        }
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postings_preserve_word_membership_and_capped_nonoverlapping_frequency() {
        let mut fragments = vec![None; 600];
        fragments[0] = Some(vec![
            Arc::new("ana BANANANA cat concatenate concatenate".into()),
            Arc::new("Cat-cat cat CATS İ İSTANBUL ΑΣ’Α".into()),
        ]);
        fragments[44] = Some(vec![Arc::new("banana cats".into())]);
        fragments[599] = Some(vec![Arc::new("ANA Αθήνα CAFÉ café ΑΣ ΑΣ’Α".into())]);
        let queries = [
            "ana cat banana",
            "İ café ΑΣ",
            "cat cats",
            "ANA ana nonesuch",
            "!",
        ];
        let index = FragmentLexicalIndex::build(&fragments).unwrap();
        for query in queries {
            let terms = lexical_terms(query);
            let expected = fragments
                .iter()
                .map(|turns| {
                    let Some(turns) = turns else {
                        return LexicalMatchCounts::default();
                    };
                    let text = turns
                        .iter()
                        .map(|turn| turn.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let lower = text.to_lowercase();
                    let words = lexical_terms(&text);
                    let mut evidence = LexicalMatchCounts::default();
                    for term in &terms {
                        if words.contains(term) {
                            let count = lower.matches(term).take(4).count();
                            evidence.matched_terms += usize::from(count > 0);
                            evidence.frequency += count;
                        }
                    }
                    evidence
                })
                .collect::<Vec<_>>();
            assert_eq!(index.counts(&terms), expected, "{query}");
        }
        let stats = index.stats();
        assert_eq!(
            stats.offset_bytes,
            (stats.terms + 1) * size_of::<TermOffset>()
        );
        assert!(stats.posting_bytes > stats.postings);
    }
}
