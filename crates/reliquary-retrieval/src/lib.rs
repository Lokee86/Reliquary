//! Deterministic Retrieval policies over caller-supplied owner snapshots.
//!
//! This crate evaluates supplied text, vector scores, graph relations and
//! community memberships. It does not open or mutate any owner store.

use std::collections::{HashMap, HashSet};

use reliquary_archive::ContentId;
use reliquary_memory_core::MemoryId;
use reliquary_vector_store::VectorGenerationId;

pub mod community_routing;
pub mod graph_traversal;
pub mod memory_retrieval;
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

/// Stable identity and owner-supplied source bounds for one searchable fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentSnapshot {
    pub id: [u8; 32],
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub archive_version: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexicalHit {
    pub fragment: FragmentSnapshot,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticSearchHit {
    pub fragment: FragmentSnapshot,
    pub score: f64,
    pub ordinal: u64,
    pub generation_id: VectorGenerationId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchCandidate {
    pub fragment: FragmentSnapshot,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub combined_score: f64,
    pub generation_id: Option<VectorGenerationId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HybridRankResult {
    Complete(Vec<SearchCandidate>),
    NeedsOverlap(FragmentSnapshot, FragmentSnapshot),
}

struct GreedyRound {
    candidate_index: usize,
    prior_index: usize,
    score: f64,
    best_index: usize,
    best_score: f64,
}

/// Greedy maximal marginal relevance ranker. Missing same-conversation overlap
/// is requested from the caller, allowing bounded hydration outside owner locks.
pub struct HybridRanker {
    remaining: Vec<SearchCandidate>,
    selected: Vec<SearchCandidate>,
    limit: usize,
    result_limit: usize,
    round: Option<GreedyRound>,
    finished: bool,
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
                .then_with(|| left.fragment.id.cmp(&right.fragment.id))
        });
        let remaining = deduplicate_search_candidates(remaining);
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
        Self {
            selected: Vec::with_capacity(limit.min(remaining.len())),
            remaining,
            limit,
            result_limit,
            round: None,
            finished: false,
        }
    }

    pub fn advance(&mut self, overlaps: &HashMap<([u8; 32], [u8; 32]), f64>) -> HybridRankResult {
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
                    && candidate.fragment.id < self.remaining[round.best_index].fragment.id)
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

pub fn lexical_score(text: &str, terms: &[String]) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryLexicalHit {
    pub memory_id: MemoryId,
    pub score: f64,
}

/// Minimal immutable attachment metadata needed for filename retrieval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSnapshot {
    pub id: [u8; 32],
    pub content_id: ContentId,
    pub filename: String,
    pub mime_type: Option<String>,
    pub byte_length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileSearchHit {
    pub file: FileSnapshot,
    pub score: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemoryCandidateEvidence {
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryCandidate {
    pub memory_id: MemoryId,
    pub evidence: MemoryCandidateEvidence,
}

/// Complete per-owner lexical/semantic eligibility union; no result limit is applied.
pub fn union_memory_evidence(
    lexical: impl IntoIterator<Item = MemoryLexicalHit>,
    semantic: impl IntoIterator<Item = memory_retrieval::MemoryRetrievalHit>,
) -> Vec<MemoryCandidate> {
    let mut evidence = std::collections::BTreeMap::<MemoryId, MemoryCandidateEvidence>::new();
    for hit in lexical {
        evidence.entry(hit.memory_id).or_default().lexical_score = Some(hit.score);
    }
    for hit in semantic {
        evidence.entry(hit.memory_id).or_default().semantic_score = Some(hit.score);
    }
    evidence
        .into_iter()
        .map(|(memory_id, evidence)| MemoryCandidate {
            memory_id,
            evidence,
        })
        .collect()
}

/// Disposable lexical snapshot index. Callers supply already-authorized text.
#[derive(Default)]
pub struct LexicalIndex {
    fragments: Vec<(FragmentSnapshot, String)>,
    raw_turns: Vec<(FragmentSnapshot, i64, String)>,
    files: Vec<FileSnapshot>,
    memories: HashMap<MemoryId, (String, bool)>,
}

impl LexicalIndex {
    pub fn add_fragment(&mut self, fragment: FragmentSnapshot, text: String) {
        self.fragments.push((fragment, text));
    }

    pub fn add_raw_turn(&mut self, source: FragmentSnapshot, timestamp_ns: i64, text: String) {
        self.raw_turns.push((source, timestamp_ns, text));
    }

    pub fn add_file(&mut self, file: FileSnapshot) {
        self.files.push(file);
    }

    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }
    pub fn raw_turn_count(&self) -> usize {
        self.raw_turns.len()
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn add_memory(&mut self, memory_id: MemoryId, text: String, active: bool) {
        self.memories.insert(memory_id, (text, active));
    }

    pub fn search(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<_> = self
            .fragments
            .iter()
            .filter_map(|(fragment, text)| {
                let score = lexical_score(text, terms);
                (score > 0.0).then_some(LexicalHit {
                    fragment: fragment.clone(),
                    score,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| {
                    right
                        .fragment
                        .archive_version
                        .cmp(&left.fragment.archive_version)
                })
                .then_with(|| left.fragment.id.cmp(&right.fragment.id))
        });
        hits.truncate(limit);
        hits
    }

    pub fn complete_memory_matches(&self, terms: &[String]) -> Vec<MemoryLexicalHit> {
        if terms.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<_> = self
            .memories
            .iter()
            .filter_map(|(memory_id, (text, active))| {
                if !active {
                    return None;
                }
                let score = lexical_score(text, terms);
                (score > 0.0).then_some(MemoryLexicalHit {
                    memory_id: *memory_id,
                    score,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.memory_id.cmp(&right.memory_id))
        });
        hits
    }

    /// Search separately hydrated source turns; returned hits retain the exact
    /// owner-provided conversation and node bounds needed for source resolution.
    pub fn search_raw(&self, terms: &[String], limit: usize) -> Vec<LexicalHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<_> = self
            .raw_turns
            .iter()
            .filter_map(|(source, timestamp_ns, text)| {
                let score = lexical_score(text, terms);
                (score > 0.0).then_some((
                    LexicalHit {
                        fragment: source.clone(),
                        score,
                    },
                    *timestamp_ns,
                ))
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .0
                .score
                .total_cmp(&left.0.score)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.0.fragment.id.cmp(&right.0.fragment.id))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(hit, _)| hit).collect()
    }

    /// Search only the supplied filenames; file bytes are neither opened nor inferred.
    pub fn search_files(&self, terms: &[String], limit: usize) -> Vec<FileSearchHit> {
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<_> = self
            .files
            .iter()
            .filter_map(|file| {
                let score = lexical_score(&file.filename, terms);
                (score > 0.0).then_some(FileSearchHit {
                    file: file.clone(),
                    score,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.file.filename.cmp(&right.file.filename))
                .then_with(|| left.file.id.cmp(&right.file.id))
        });
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment(id: u8, start: &str, end: &str) -> FragmentSnapshot {
        FragmentSnapshot {
            id: [id; 32],
            conversation_id: "conv".into(),
            start_node_id: start.into(),
            end_node_id: end.into(),
            archive_version: 1,
        }
    }

    #[test]
    fn lexical_contract_preserves_unicode_dedup_and_frequency_cap() {
        assert_eq!(
            lexical_terms("Été—ALPHA été 你好"),
            ["été", "alpha", "你好"]
        );
        let terms = lexical_terms("Alpha, beta alpha!");
        assert!((lexical_score("alpha alpha alpha", &terms) - 0.5375).abs() < 1e-9);
        assert_eq!(lexical_score("nothing", &terms), 0.0);
    }

    #[test]
    fn hybrid_request_validation_and_weights_match_reference() {
        assert_eq!(
            validate_hybrid_request(" ", RetrievalConfig::default()),
            Err(HybridPolicyError::EmptyQuery)
        );
        assert_eq!(
            validate_hybrid_request("query", RetrievalConfig::default()),
            Ok((0.45, 0.55))
        );
        let invalid = RetrievalConfig {
            candidate_limit: 0,
            ..RetrievalConfig::default()
        };
        assert_eq!(
            validate_hybrid_request("query", invalid),
            Err(HybridPolicyError::InvalidConfig)
        );
    }

    #[test]
    fn hybrid_merge_combines_lanes_and_uses_reference_dedup_key() {
        let config = RetrievalConfig {
            candidate_limit: 5,
            result_limit: 5,
            ..RetrievalConfig::default()
        };
        let lexical = [LexicalHit {
            fragment: fragment(1, "n1", "n2"),
            score: 0.8,
        }];
        let semantic = [SemanticSearchHit {
            fragment: fragment(1, "n1", "n2"),
            score: 0.6,
            ordinal: 0,
            generation_id: VectorGenerationId([9; 32]),
        }];
        let mut ranker = HybridRanker::new(&lexical, &semantic, config, (0.45, 0.55));
        let HybridRankResult::Complete(selected) = ranker.advance(&HashMap::new()) else {
            panic!("no overlap needed")
        };
        assert_eq!(selected.len(), 1);
        assert!((selected[0].combined_score - 0.69).abs() < 1e-12);
        assert_eq!(selected[0].generation_id, Some(VectorGenerationId([9; 32])));
    }

    #[test]
    fn hybrid_ranker_requests_missing_overlap_and_is_retryable() {
        let candidates = vec![
            SearchCandidate {
                fragment: fragment(1, "a", "b"),
                lexical_score: 0.9,
                semantic_score: 0.0,
                combined_score: 0.9,
                generation_id: None,
            },
            SearchCandidate {
                fragment: fragment(2, "b", "c"),
                lexical_score: 0.8,
                semantic_score: 0.0,
                combined_score: 0.8,
                generation_id: None,
            },
        ];
        let mut ranker = HybridRanker::from_candidates(candidates, 10, 10);
        let pair = match ranker.advance(&HashMap::new()) {
            HybridRankResult::NeedsOverlap(left, right) => (left.id, right.id),
            _ => panic!("expected overlap request"),
        };
        let overlaps = [(pair, 0.95)].into_iter().collect();
        let HybridRankResult::Complete(selected) = ranker.advance(&overlaps) else {
            panic!("expected completion")
        };
        assert_eq!(
            selected.iter().map(|c| c.fragment.id).collect::<Vec<_>>(),
            [[1; 32], [2; 32]]
        );
    }

    #[test]
    fn lexical_index_sorts_by_score_then_recency_then_stable_id() {
        let mut index = LexicalIndex::default();
        let mut old = fragment(2, "a", "b");
        old.archive_version = 1;
        let mut recent = fragment(3, "c", "d");
        recent.archive_version = 2;
        index.add_fragment(old, "alpha beta".into());
        index.add_fragment(recent, "alpha beta".into());
        let hits = index.search(&lexical_terms("alpha beta"), 5);
        assert_eq!(
            hits.iter()
                .map(|hit| hit.fragment.id[0])
                .collect::<Vec<_>>(),
            [3, 2]
        );
    }

    #[test]
    fn candidate_union_retains_lexical_only_semantic_only_and_overlap() {
        let union = union_memory_evidence(
            [
                MemoryLexicalHit {
                    memory_id: MemoryId([2; 32]),
                    score: 0.6,
                },
                MemoryLexicalHit {
                    memory_id: MemoryId([1; 32]),
                    score: 0.4,
                },
            ],
            [
                memory_retrieval::MemoryRetrievalHit {
                    memory_id: MemoryId([2; 32]),
                    score: 0.9,
                },
                memory_retrieval::MemoryRetrievalHit {
                    memory_id: MemoryId([3; 32]),
                    score: 0.8,
                },
            ],
        );
        assert_eq!(
            union
                .iter()
                .map(|candidate| candidate.memory_id)
                .collect::<Vec<_>>(),
            [MemoryId([1; 32]), MemoryId([2; 32]), MemoryId([3; 32])]
        );
        assert_eq!(
            union[1].evidence,
            MemoryCandidateEvidence {
                lexical_score: Some(0.6),
                semantic_score: Some(0.9)
            }
        );
    }

    #[test]
    fn raw_turn_hits_keep_source_bounds_and_use_timestamp_tie_break() {
        let mut index = LexicalIndex::default();
        let mut earlier = fragment(2, "n2", "n2");
        earlier.conversation_id = "conversation-a".into();
        let mut later = fragment(1, "n1", "n1");
        later.conversation_id = "conversation-b".into();
        index.add_raw_turn(earlier, 10, "alpha beta".into());
        index.add_raw_turn(later, 20, "alpha beta".into());
        let hits = index.search_raw(&lexical_terms("alpha beta"), 10);
        assert_eq!(hits[0].fragment.id, [1; 32]);
        assert_eq!(hits[0].fragment.conversation_id, "conversation-b");
        assert_eq!(hits[0].fragment.start_node_id, "n1");
        assert_eq!(hits[1].fragment.id, [2; 32]);
    }

    #[test]
    fn file_search_scores_filename_only_and_breaks_ties_by_identity() {
        let mut index = LexicalIndex::default();
        index.add_file(FileSnapshot {
            id: [2; 32],
            content_id: ContentId([4; 32]),
            filename: "notes-alpha.txt".into(),
            mime_type: Some("text/plain".into()),
            byte_length: 99,
        });
        index.add_file(FileSnapshot {
            id: [1; 32],
            content_id: ContentId([3; 32]),
            filename: "notes-alpha.md".into(),
            mime_type: Some("text/markdown".into()),
            byte_length: 10,
        });
        index.add_file(FileSnapshot {
            id: [0; 32],
            content_id: ContentId([2; 32]),
            filename: "unrelated.txt".into(),
            mime_type: None,
            byte_length: 20,
        });
        let hits = index.search_files(&lexical_terms("notes alpha"), 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].file.id, [1; 32]);
        assert_eq!(hits[1].file.id, [2; 32]);
        assert_eq!(hits[0].file.content_id, ContentId([3; 32]));
    }
}

pub mod agentic;
