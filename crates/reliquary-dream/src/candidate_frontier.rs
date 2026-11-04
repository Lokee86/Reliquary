//! Bounded streaming frontier equivalent to full-set Dream lane ranking.
//! Each lane only contributes its top-ranked members to the fused candidate set.
use crate::{DreamCandidateConfig, RankedCandidate, rank_lanes, select_candidates};
use reliquary_memory_core::MemoryId;
use std::collections::BTreeMap;

/// Keeps at most the selected semantic, prior, lexical and temporal lane
/// frontiers while scanning caller-authorized candidates. No owner store,
/// embedding index or model endpoint is accessed by this pure component.
pub struct DreamCandidateFrontier {
    config: DreamCandidateConfig,
    source_time_ns: Option<i64>,
    semantic: Vec<RankedCandidate>,
    prior: Vec<RankedCandidate>,
    lexical: Vec<RankedCandidate>,
    temporal: Vec<RankedCandidate>,
}

impl DreamCandidateFrontier {
    pub fn new(source_time_ns: Option<i64>, config: DreamCandidateConfig) -> Self {
        Self {
            config,
            source_time_ns,
            semantic: Vec::new(),
            prior: Vec::new(),
            lexical: Vec::new(),
            temporal: Vec::new(),
        }
    }

    pub fn push(&mut self, candidate: RankedCandidate) {
        let semantic = candidate
            .semantic_score
            .filter(|score| score.is_finite() && *score > 0.0);
        if semantic.is_some() {
            keep_best(
                &mut self.semantic,
                &candidate,
                self.config.semantic_limit,
                semantic_score,
            );
            if self
                .source_time_ns
                .zip(candidate.source_timestamp_ns)
                .is_some_and(|(source, other)| other < source)
            {
                // Prior comes from the entire positive semantic lane, not
                // just its truncated semantic_limit candidates.
                keep_best(
                    &mut self.prior,
                    &candidate,
                    self.config.prior_semantic_quota,
                    semantic_score,
                );
            }
        }
        if candidate.lexical_score.is_finite() && candidate.lexical_score > 0.0 {
            keep_best(
                &mut self.lexical,
                &candidate,
                self.config.lexical_limit,
                |item| item.lexical_score,
            );
        }
        if candidate.temporal_score.is_finite() && candidate.temporal_score > 0.0 {
            keep_best(
                &mut self.temporal,
                &candidate,
                self.config.temporal_limit,
                |item| item.temporal_score,
            );
        }
    }

    /// Maximum number of candidate entries retained across lanes.
    pub fn retained_entries(&self) -> usize {
        self.semantic.len() + self.prior.len() + self.lexical.len() + self.temporal.len()
    }

    pub fn finish(self) -> Vec<RankedCandidate> {
        let mut union: BTreeMap<MemoryId, RankedCandidate> = BTreeMap::new();
        for item in self
            .semantic
            .into_iter()
            .chain(self.prior)
            .chain(self.lexical)
            .chain(self.temporal)
        {
            union.entry(item.id).or_insert(item);
        }
        let mut candidates: Vec<_> = union.into_values().collect();
        rank_lanes(self.source_time_ns, &mut candidates, self.config);
        select_candidates(candidates, self.config)
    }
}

fn semantic_score(item: &RankedCandidate) -> f64 {
    item.semantic_score.unwrap_or(f64::NEG_INFINITY)
}

fn keep_best(
    top: &mut Vec<RankedCandidate>,
    candidate: &RankedCandidate,
    limit: usize,
    score: fn(&RankedCandidate) -> f64,
) {
    if limit == 0 {
        return;
    }
    top.push(candidate.clone());
    top.sort_by(|a, b| {
        score(b)
            .total_cmp(&score(a))
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    top.truncate(limit);
}

#[cfg(test)]
#[path = "candidate_frontier_tests.rs"]
mod tests;
