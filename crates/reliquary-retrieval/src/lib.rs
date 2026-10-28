//! Retrieval ownership boundary.
//!
//! Deterministic retrieval policies over caller-supplied text.

use std::collections::{HashMap, HashSet};

use reliquary_archive::{FileSearchHit, Fragment, StoredFile};
use reliquary_memory_core::MemoryId;
use reliquary_vector_store::VectorGenerationId;

pub mod community_routing;
pub mod graph_traversal;
pub mod memory_candidates;
pub mod memory_retrieval;
pub mod ordered_fragment_search;
pub mod semantic_scoring;

pub const DEFAULT_LEXICAL_WEIGHT: f64 = 0.45;
pub const DEFAULT_SEMANTIC_WEIGHT: f64 = 0.55;
pub const DEFAULT_SEARCH_CANDIDATE_LIMIT: usize = 30;
pub const DEFAULT_SEARCH_RESULT_LIMIT: usize = 10;
pub const MAX_SEMANTIC_SEARCH_LIMIT: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetrievalConfig {
    pub candidate_limit: usize,
    pub result_limit: usize,
    pub lexical_weight: f64,
    pub semantic_weight: f64,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            candidate_limit: DEFAULT_SEARCH_CANDIDATE_LIMIT,
            result_limit: DEFAULT_SEARCH_RESULT_LIMIT,
            lexical_weight: DEFAULT_LEXICAL_WEIGHT,
            semantic_weight: DEFAULT_SEMANTIC_WEIGHT,
        }
    }
}

impl RetrievalConfig {
    pub fn normalized_weights(self) -> Option<(f64, f64)> {
        if self.candidate_limit == 0
            || self.result_limit == 0
            || self.result_limit > self.candidate_limit
            || self.lexical_weight <= 0.0
            || self.semantic_weight <= 0.0
            || !self.lexical_weight.is_finite()
            || !self.semantic_weight.is_finite()
        {
            return None;
        }
        let total = self.lexical_weight + self.semantic_weight;
        total
            .is_finite()
            .then_some((self.lexical_weight / total, self.semantic_weight / total))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HybridPolicyError {
    EmptyQuery,
    InvalidConfig,
}

/// Validates the pure hybrid-search request policy and returns normalized lane weights.
pub fn validate_hybrid_request(
    query: &str,
    config: RetrievalConfig,
) -> Result<(f64, f64), HybridPolicyError> {
    if query.trim().is_empty() {
        return Err(HybridPolicyError::EmptyQuery);
    }
    let weights = config
        .normalized_weights()
        .ok_or(HybridPolicyError::InvalidConfig)?;
    if config.candidate_limit > MAX_SEMANTIC_SEARCH_LIMIT {
        return Err(HybridPolicyError::InvalidConfig);
    }
    Ok(weights)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchCandidate {
    pub fragment: Fragment,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub combined_score: f64,
    pub generation_id: Option<VectorGenerationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticSearchHit {
    pub fragment: Fragment,
    pub score: f64,
    pub ordinal: u64,
    pub generation_id: VectorGenerationId,
}

pub enum HybridRankResult {
    Complete(Vec<SearchCandidate>),
    NeedsOverlap(Fragment, Fragment),
}

pub struct HybridRanker {
    remaining: Vec<SearchCandidate>,
    selected: Vec<SearchCandidate>,
    limit: usize,
    result_limit: usize,
    round: Option<GreedyRound>,
    finished: bool,
}

struct GreedyRound {
    candidate_index: usize,
    prior_index: usize,
    score: f64,
    best_index: usize,
    best_score: f64,
}

impl HybridRanker {
    pub fn new(
        lexical: &[LexicalHit],
        semantic: &[SemanticSearchHit],
        config: RetrievalConfig,
        weights: (f64, f64),
    ) -> Self {
        let mut by_id = HashMap::new();
        for hit in lexical {
            by_id.insert(
                hit.fragment.id,
                SearchCandidate {
                    fragment: hit.fragment.clone(),
                    lexical_score: hit.score,
                    semantic_score: 0.0,
                    combined_score: hit.score,
                    generation_id: None,
                },
            );
        }
        for hit in semantic {
            let candidate = by_id
                .entry(hit.fragment.id)
                .or_insert_with(|| SearchCandidate {
                    fragment: hit.fragment.clone(),
                    lexical_score: 0.0,
                    semantic_score: 0.0,
                    combined_score: 0.0,
                    generation_id: None,
                });
            candidate.semantic_score = hit.score;
            candidate.generation_id = Some(hit.generation_id);
        }
        let mut remaining: Vec<_> = by_id.into_values().collect();
        for candidate in &mut remaining {
            candidate.combined_score =
                combine_hybrid_scores(candidate.lexical_score, candidate.semantic_score, weights);
        }
        remaining.sort_by(|left, right| {
            right
                .combined_score
                .total_cmp(&left.combined_score)
                .then_with(|| left.fragment.id.0.cmp(&right.fragment.id.0))
        });
        remaining = deduplicate_search_candidates(remaining);
        Self::from_ranked_candidates(remaining, config.candidate_limit, config.result_limit)
    }

    pub fn from_candidates(
        candidates: Vec<SearchCandidate>,
        limit: usize,
        result_limit: usize,
    ) -> Self {
        Self::from_ranked_candidates(candidates, limit, result_limit)
    }

    fn from_ranked_candidates(
        remaining: Vec<SearchCandidate>,
        limit: usize,
        result_limit: usize,
    ) -> Self {
        let selected = Vec::with_capacity(limit.min(remaining.len()));
        Self {
            remaining,
            selected,
            limit,
            result_limit,
            round: None,
            finished: false,
        }
    }

    pub fn advance(
        &mut self,
        overlaps: &HashMap<(reliquary_archive::FragmentId, reliquary_archive::FragmentId), f64>,
    ) -> HybridRankResult {
        while !self.remaining.is_empty() && self.selected.len() < self.limit {
            if self.round.is_none() {
                self.round = Some(GreedyRound {
                    candidate_index: 0,
                    prior_index: 0,
                    score: self.remaining[0].combined_score,
                    best_index: 0,
                    best_score: f64::NEG_INFINITY,
                });
            }
            let round = self.round.as_mut().expect("greedy round initialized");
            if round.candidate_index >= self.remaining.len() {
                self.selected.push(self.remaining.remove(round.best_index));
                self.round = None;
                continue;
            }
            let candidate = &self.remaining[round.candidate_index];
            if round.prior_index < self.selected.len() {
                let prior = &self.selected[round.prior_index];
                if candidate.fragment.conversation_id == prior.fragment.conversation_id {
                    let pair = (candidate.fragment.id, prior.fragment.id);
                    let reverse = (prior.fragment.id, candidate.fragment.id);
                    let Some(overlap) = overlaps.get(&pair).or_else(|| overlaps.get(&reverse))
                    else {
                        return HybridRankResult::NeedsOverlap(
                            candidate.fragment.clone(),
                            prior.fragment.clone(),
                        );
                    };
                    round.score *= 1.0 - overlap;
                }
                round.prior_index += 1;
                continue;
            }
            if round.score > round.best_score
                || (round.score == round.best_score
                    && candidate.fragment.id.0 < self.remaining[round.best_index].fragment.id.0)
            {
                round.best_index = round.candidate_index;
                round.best_score = round.score;
            }
            round.candidate_index += 1;
            round.prior_index = 0;
            if let Some(next) = self.remaining.get(round.candidate_index) {
                round.score = next.combined_score;
            }
        }
        if self.finished {
            return HybridRankResult::Complete(Vec::new());
        }
        self.finished = true;
        self.selected.truncate(self.result_limit);
        HybridRankResult::Complete(std::mem::take(&mut self.selected))
    }
}

pub fn combine_hybrid_scores(lexical: f64, semantic: f64, weights: (f64, f64)) -> f64 {
    match (valid_score(lexical), valid_score(semantic)) {
        (true, true) => weights.0 * lexical + weights.1 * semantic,
        (true, false) => lexical,
        (false, true) => semantic,
        (false, false) => 0.0,
    }
}

pub fn deduplicate_search_candidates(candidates: Vec<SearchCandidate>) -> Vec<SearchCandidate> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            seen.insert((
                candidate.fragment.conversation_id.clone(),
                candidate.fragment.start_node_id.clone(),
                candidate.fragment.end_node_id.clone(),
            ))
        })
        .collect()
}

fn valid_score(score: f64) -> bool {
    score > 0.0 && score.is_finite()
}

#[derive(Clone, Debug)]
pub struct LexicalHit {
    pub fragment: Fragment,
    pub score: f64,
}

/// Disposable posting indexes. Callers enumerate and hydrate source records, then add text here.
#[derive(Default)]
pub struct LexicalIndex {
    fragments: Vec<(Fragment, u64)>,
    postings: HashMap<String, Vec<Posting>>,
    raw_turns: Vec<(Fragment, i64)>,
    raw_postings: HashMap<String, Vec<Posting>>,
    files: Vec<StoredFile>,
    file_postings: HashMap<String, Vec<Posting>>,
    memory_version: u64,
    memories: Vec<MemoryId>,
    memory_active: Vec<bool>,
    memory_slots: HashMap<MemoryId, usize>,
    memory_postings: HashMap<String, Vec<Posting>>,
}

#[derive(Clone, Copy)]
struct Posting {
    slot: usize,
    frequency: u8,
}
#[derive(Clone, Copy, Default)]
struct ScoreParts {
    matched_terms: usize,
    frequency: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryLexicalHit {
    pub memory_id: MemoryId,
    pub score: f64,
}

impl LexicalIndex {
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    pub fn raw_turn_count(&self) -> usize {
        self.raw_turns.len()
    }

    pub fn add_fragment(&mut self, fragment: Fragment, archive_version: u64, text: &str) {
        let slot = self.fragments.len();
        add_terms(&mut self.postings, text, slot);
        self.fragments.push((fragment, archive_version));
    }

    pub fn add_file(&mut self, file: StoredFile) {
        let slot = self.files.len();
        add_terms(&mut self.file_postings, &file.filename, slot);
        self.files.push(file);
    }

    pub fn add_raw_turn(&mut self, fragment: Fragment, timestamp_ns: i64, text: &str) {
        let slot = self.raw_turns.len();
        add_terms(&mut self.raw_postings, text, slot);
        self.raw_turns.push((fragment, timestamp_ns));
    }

    pub fn search(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores: HashMap<usize, ScoreParts> = HashMap::new();
        accumulate(terms, &self.postings, &mut scores);
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| {
                (
                    LexicalHit {
                        fragment: self.fragments[slot].0.clone(),
                        score: score(parts, terms.len()),
                    },
                    self.fragments[slot].1,
                )
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            b.0.score
                .total_cmp(&a.0.score)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.fragment.id.0.cmp(&b.0.fragment.id.0))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(hit, _)| hit).collect()
    }

    pub fn search_raw(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores = HashMap::new();
        accumulate(terms, &self.raw_postings, &mut scores);
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| {
                (
                    LexicalHit {
                        fragment: self.raw_turns[slot].0.clone(),
                        score: score(parts, terms.len()),
                    },
                    self.raw_turns[slot].1,
                )
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            b.0.score
                .total_cmp(&a.0.score)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.fragment.id.0.cmp(&b.0.fragment.id.0))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(hit, _)| hit).collect()
    }

    pub fn search_files(&self, terms: &[String], limit: usize) -> Vec<FileSearchHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scores = HashMap::new();
        for term in terms {
            if let Some(posts) = self.file_postings.get(term) {
                for p in posts {
                    add_score(&mut scores, p.slot, p.frequency);
                }
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| FileSearchHit {
                file: self.files[slot].clone(),
                score: score(parts, terms.len()),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.file.filename.cmp(&b.file.filename))
                .then_with(|| a.file.id.0.cmp(&b.file.id.0))
        });
        hits.truncate(limit);
        hits
    }

    pub fn add_memory(&mut self, id: MemoryId, title: &str, content: &str) {
        if self.memory_slots.contains_key(&id) {
            return;
        }
        let slot = self.memories.len();
        add_terms(
            &mut self.memory_postings,
            &format!("{title}\n\n{content}"),
            slot,
        );
        self.memory_slots.insert(id, slot);
        self.memories.push(id);
        self.memory_active.push(true);
    }
    pub fn has_memory(&self, id: MemoryId) -> bool {
        self.memory_slots.contains_key(&id)
    }
    pub fn memory_ids(&self) -> impl Iterator<Item = MemoryId> + '_ {
        self.memory_slots.keys().copied()
    }
    pub fn set_memory_active(&mut self, id: MemoryId, active: bool) {
        if let Some(slot) = self.memory_slots.get(&id) {
            self.memory_active[*slot] = active;
        }
    }
    pub fn set_memory_version(&mut self, version: u64) {
        self.memory_version = version;
    }
    pub fn memory_version(&self) -> u64 {
        self.memory_version
    }
    pub fn memory_indexed_slots(&self) -> usize {
        self.memories.len()
    }
    pub fn reset_memory_index(&mut self) {
        self.memory_version = 0;
        self.memories.clear();
        self.memory_active.clear();
        self.memory_slots.clear();
        self.memory_postings.clear();
    }
    pub fn complete_memory_matches(&self, terms: &[String]) -> Vec<MemoryLexicalHit> {
        if terms.is_empty() {
            return Vec::new();
        }
        let mut scores = HashMap::new();
        for term in terms {
            if let Some(posts) = self.memory_postings.get(term) {
                for p in posts {
                    if self.memory_active[p.slot] {
                        add_score(&mut scores, p.slot, p.frequency);
                    }
                }
            }
        }
        let mut hits = scores
            .into_iter()
            .map(|(slot, parts)| MemoryLexicalHit {
                memory_id: self.memories[slot],
                score: score(parts, terms.len()),
            })
            .collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.memory_id.0.cmp(&b.memory_id.0))
        });
        hits
    }
    pub fn search_memories(&self, terms: &[String], limit: usize) -> Vec<MemoryLexicalHit> {
        let mut hits = self.complete_memory_matches(terms);
        hits.truncate(limit);
        hits
    }
}

fn add_terms(map: &mut HashMap<String, Vec<Posting>>, text: &str, slot: usize) {
    for (term, count) in term_counts(text) {
        map.entry(term).or_default().push(Posting {
            slot,
            frequency: count.min(4) as u8,
        });
    }
}
fn accumulate(
    terms: &[String],
    postings: &HashMap<String, Vec<Posting>>,
    scores: &mut HashMap<usize, ScoreParts>,
) {
    for term in terms {
        if let Some(posts) = postings.get(term) {
            for p in posts {
                add_score(scores, p.slot, p.frequency);
            }
        }
    }
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

/// Normalize text into distinct lowercase alphanumeric terms, preserving first-seen order.
pub fn lexical_terms(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, seen: &mut HashSet<String>, terms: &mut Vec<String>| {
        if word.is_empty() {
            return;
        }
        let term = word.to_lowercase();
        word.clear();
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    };
    for current in value.chars() {
        if current.is_alphanumeric() {
            word.push(current);
        } else {
            flush(&mut word, &mut seen, &mut terms);
        }
    }
    flush(&mut word, &mut seen, &mut terms);
    terms
}

/// Count distinct normalized terms using the legacy substring frequency semantics.
pub fn term_counts(value: &str) -> HashMap<String, usize> {
    let lower = value.to_lowercase();
    lexical_terms(value)
        .into_iter()
        .map(|term| {
            let count = lower.matches(&term).count();
            (term, count)
        })
        .collect()
}

/// Score text using the legacy lexical coverage and capped-frequency formula.
pub fn lexical_score(text: &str, terms: &[String]) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_unicode_and_deduplicates_in_first_seen_order() {
        assert_eq!(
            lexical_terms("Été—ALPHA été 你好"),
            vec!["été", "alpha", "你好"]
        );
    }

    #[test]
    fn retains_legacy_coverage_and_frequency_caps() {
        let terms = lexical_terms("Alpha, beta alpha!");
        assert_eq!(terms, vec!["alpha", "beta"]);
        assert!((lexical_score("alpha alpha alpha", &terms) - 0.5375).abs() < 1e-9);
        assert_eq!(lexical_score("nothing", &terms), 0.0);
    }

    #[test]
    fn hybrid_request_validation_preserves_query_then_config_order() {
        let invalid = RetrievalConfig {
            candidate_limit: 0,
            ..RetrievalConfig::default()
        };
        assert_eq!(
            validate_hybrid_request("  ", invalid),
            Err(HybridPolicyError::EmptyQuery)
        );
        assert_eq!(
            validate_hybrid_request("query", invalid),
            Err(HybridPolicyError::InvalidConfig)
        );
        assert_eq!(
            validate_hybrid_request("query", RetrievalConfig::default()),
            Ok((0.45, 0.55))
        );
    }
}
