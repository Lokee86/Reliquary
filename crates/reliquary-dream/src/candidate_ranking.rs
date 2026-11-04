use crate::DreamCandidateConfig;
use reliquary_memory_core::{MemoryBody, MemoryId, MemoryRecord};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct RankedCandidate {
    pub id: MemoryId,
    pub source_timestamp_ns: Option<i64>,
    pub semantic_score: Option<f64>,
    pub lexical_score: f64,
    pub semantic_rank: Option<usize>,
    pub prior_rank: Option<usize>,
    pub lexical_rank: Option<usize>,
    pub temporal_score: f64,
    pub temporal_rank: Option<usize>,
    pub fused_score: f64,
}

/// Reference lexical lane score over the owner-supplied records and bodies.
pub fn lexical_score(
    source: &MemoryRecord,
    source_body: &MemoryBody,
    candidate: &MemoryRecord,
    candidate_body: &MemoryBody,
) -> f64 {
    fn tokens(body: &MemoryBody) -> HashSet<String> {
        format!("{} {}", body.title, body.content)
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|token| token.len() > 1)
            .map(str::to_lowercase)
            .collect()
    }
    let source_tokens = tokens(source_body);
    let candidate_tokens = tokens(candidate_body);
    let union = source_tokens.union(&candidate_tokens).count();
    let mut score = if union == 0 {
        0.0
    } else {
        source_tokens.intersection(&candidate_tokens).count() as f64 / union as f64
    };
    if source.category == candidate.category {
        score += 0.08;
    }
    if source.memory_type == candidate.memory_type {
        score += 0.08;
    }
    if source.parent_id.is_some() && source.parent_id == candidate.parent_id {
        score += 0.12;
    }
    score
}

pub fn rank_lanes(
    source_timestamp_ns: Option<i64>,
    candidates: &mut [RankedCandidate],
    config: DreamCandidateConfig,
) {
    for candidate in candidates.iter_mut() {
        candidate.semantic_rank = None;
        candidate.prior_rank = None;
        candidate.lexical_rank = None;
        candidate.temporal_rank = None;
        candidate.fused_score = 0.0;
    }
    let mut semantic: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            candidate
                .semantic_score
                .filter(|score| score.is_finite() && *score > 0.0)
                .map(|score| (index, score))
        })
        .collect();
    semantic.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| candidates[a.0].id.0.cmp(&candidates[b.0].id.0))
    });
    for (rank, (index, _)) in semantic.iter().take(config.semantic_limit).enumerate() {
        candidates[*index].semantic_rank = Some(rank + 1);
    }

    if let Some(source_time) = source_timestamp_ns {
        let mut prior: Vec<_> = semantic
            .iter()
            .copied()
            .filter(|(index, _)| {
                candidates[*index]
                    .source_timestamp_ns
                    .is_some_and(|timestamp| timestamp < source_time)
            })
            .collect();
        prior.truncate(config.prior_semantic_quota);
        for (rank, (index, _)) in prior.iter().enumerate() {
            candidates[*index].prior_rank = Some(rank + 1);
        }
    }

    let mut lexical: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.lexical_score.is_finite() && candidate.lexical_score > 0.0
        })
        .map(|(index, candidate)| (index, candidate.lexical_score))
        .collect();
    lexical.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| candidates[a.0].id.0.cmp(&candidates[b.0].id.0))
    });
    for (rank, (index, _)) in lexical.iter().take(config.lexical_limit).enumerate() {
        candidates[*index].lexical_rank = Some(rank + 1);
    }

    let mut temporal: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.temporal_score.is_finite() && candidate.temporal_score > 0.0
        })
        .map(|(index, candidate)| (index, candidate.temporal_score))
        .collect();
    temporal.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| candidates[a.0].id.0.cmp(&candidates[b.0].id.0))
    });
    for (rank, (index, _)) in temporal.iter().take(config.temporal_limit).enumerate() {
        candidates[*index].temporal_rank = Some(rank + 1);
    }

    for candidate in candidates {
        candidate.fused_score = rrf(candidate.semantic_rank, 1.0)
            + rrf(candidate.prior_rank, 0.6)
            + rrf(candidate.lexical_rank, 0.35)
            + rrf(candidate.temporal_rank, 1.0);
    }
}

pub fn select_candidates(
    mut candidates: Vec<RankedCandidate>,
    config: DreamCandidateConfig,
) -> Vec<RankedCandidate> {
    let reserved: HashSet<_> = candidates
        .iter()
        .filter(|candidate| candidate.prior_rank.is_some())
        .map(|candidate| candidate.id)
        .collect();
    candidates.retain(|candidate| {
        candidate.semantic_rank.is_some()
            || candidate.prior_rank.is_some()
            || candidate.lexical_rank.is_some()
            || candidate.temporal_rank.is_some()
    });
    candidates.sort_by(|a, b| {
        b.fused_score
            .total_cmp(&a.fused_score)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });

    let mut selected = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| reserved.contains(&candidate.id))
    {
        if selected.len() == config.limit {
            break;
        }
        selected.push(candidate.clone());
    }
    for candidate in candidates {
        if selected.len() == config.limit {
            break;
        }
        if !selected.iter().any(|item| item.id == candidate.id) {
            selected.push(candidate);
        }
    }
    selected.sort_by(|a, b| {
        b.fused_score
            .total_cmp(&a.fused_score)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    selected
}

fn rrf(rank: Option<usize>, weight: f64) -> f64 {
    rank.map(|rank| weight / (60.0 + rank as f64))
        .unwrap_or(0.0)
}

pub fn validate_candidate_config(config: DreamCandidateConfig) -> Result<(), ()> {
    if config.limit == 0
        || config.limit > crate::MAX_DREAM_CANDIDATE_LIMIT
        || config.semantic_limit > crate::MAX_DREAM_CANDIDATE_LIMIT
        || config.lexical_limit > crate::MAX_DREAM_CANDIDATE_LIMIT
        || config.temporal_limit > crate::MAX_DREAM_CANDIDATE_LIMIT
        || config.prior_semantic_quota > config.limit
    {
        return Err(());
    }
    Ok(())
}

pub fn candidate_cosine(left: &[f32], right: &[f32]) -> Result<f64, &'static str> {
    if left.len() != right.len() || left.is_empty() {
        return Err("vector dimensions");
    }
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        if !left.is_finite() || !right.is_finite() {
            return Err("non-finite vector");
        }
        dot += *left as f64 * *right as f64;
        left_norm += (*left as f64).powi(2);
        right_norm += (*right as f64).powi(2);
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err("zero vector");
    }
    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[cfg(test)]
#[path = "candidate_ranking_tests.rs"]
mod tests;
