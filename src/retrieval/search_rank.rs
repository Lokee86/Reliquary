use crate::lexical_contract::LexicalHit;
use crate::{Cva, RetrievalConfig, SearchCandidate, SearchError, SemanticSearchHit};
use std::collections::HashMap;

impl Cva {
    pub(crate) fn rank_search_candidates(
        &self,
        lexical: Vec<LexicalHit>,
        semantic: Vec<SemanticSearchHit>,
        config: RetrievalConfig,
        weights: (f64, f64),
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        let mut by_id = HashMap::new();
        for hit in lexical {
            by_id.insert(
                hit.fragment.id,
                SearchCandidate {
                    fragment: hit.fragment,
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
        let mut candidates: Vec<_> = by_id.into_values().collect();
        for candidate in &mut candidates {
            candidate.combined_score = combine_scores_with_weights(
                candidate.lexical_score,
                candidate.semantic_score,
                weights,
            );
        }
        sort_candidates(&mut candidates);
        candidates = deduplicate_candidates(candidates);
        candidates = self.diversify_candidates(candidates, config.candidate_limit)?;
        candidates.truncate(config.result_limit);
        Ok(candidates)
    }

    pub(crate) fn diversify_candidates(
        &self,
        candidates: Vec<SearchCandidate>,
        limit: usize,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        if candidates.len() < 2 || limit == 0 {
            return Ok(candidates);
        }
        let mut remaining = candidates;
        let mut selected: Vec<SearchCandidate> = Vec::with_capacity(limit.min(remaining.len()));
        while !remaining.is_empty() && selected.len() < limit {
            let mut best = 0_usize;
            let mut best_score = f64::NEG_INFINITY;
            for (index, candidate) in remaining.iter().enumerate() {
                let mut score = candidate.combined_score;
                for prior in &selected {
                    if candidate.fragment.conversation_id == prior.fragment.conversation_id {
                        score *=
                            1.0 - self.fragment_overlap(&candidate.fragment, &prior.fragment)?;
                    }
                }
                if score > best_score
                    || (score == best_score
                        && candidate.fragment.id.0 < remaining[best].fragment.id.0)
                {
                    best = index;
                    best_score = score;
                }
            }
            selected.push(remaining.remove(best));
        }
        Ok(selected)
    }

    fn fragment_overlap(
        &self,
        left: &crate::Fragment,
        right: &crate::Fragment,
    ) -> Result<f64, SearchError> {
        if left.conversation_id != right.conversation_id {
            return Ok(0.0);
        }
        let left_nodes = self.archive.fragment_nodes_for(left)?;
        let right_nodes = self.archive.fragment_nodes_for(right)?;
        let shorter = left_nodes.len().min(right_nodes.len());
        if shorter == 0 {
            return Ok(0.0);
        }
        let right_ids: std::collections::HashSet<_> =
            right_nodes.iter().map(|node| node.id.as_str()).collect();
        let intersection = left_nodes
            .iter()
            .filter(|node| right_ids.contains(node.id.as_str()))
            .count();
        Ok((intersection as f64 / shorter as f64).min(1.0))
    }
}

#[cfg(test)]
pub(crate) fn combine_scores(lexical: f64, semantic: f64) -> f64 {
    combine_scores_with_weights(
        lexical,
        semantic,
        (
            crate::DEFAULT_LEXICAL_WEIGHT,
            crate::DEFAULT_SEMANTIC_WEIGHT,
        ),
    )
}

fn combine_scores_with_weights(lexical: f64, semantic: f64, weights: (f64, f64)) -> f64 {
    match (valid_score(lexical), valid_score(semantic)) {
        (true, true) => weights.0 * lexical + weights.1 * semantic,
        (true, false) => lexical,
        (false, true) => semantic,
        (false, false) => 0.0,
    }
}

fn valid_score(score: f64) -> bool {
    score > 0.0 && score.is_finite()
}

fn sort_candidates(candidates: &mut [SearchCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .combined_score
            .total_cmp(&left.combined_score)
            .then_with(|| left.fragment.id.0.cmp(&right.fragment.id.0))
    });
}

pub(crate) fn deduplicate_candidates(candidates: Vec<SearchCandidate>) -> Vec<SearchCandidate> {
    let mut seen = std::collections::HashSet::new();
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
