use crate::lexical_search::LexicalHit;
use crate::{Cva, RetrievalConfig, SearchCandidate, SearchError, SemanticSearchHit};

impl Cva {
    pub(crate) fn rank_search_candidates(
        &self,
        lexical: Vec<LexicalHit>,
        semantic: Vec<SemanticSearchHit>,
        config: RetrievalConfig,
        weights: (f64, f64),
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        let mut overlaps = std::collections::HashMap::new();
        let mut ranker =
            reliquary_retrieval::HybridRanker::new(&lexical, &semantic, config, weights);
        loop {
            match ranker.advance(&overlaps) {
                reliquary_retrieval::HybridRankResult::Complete(candidates) => {
                    return Ok(candidates);
                }
                reliquary_retrieval::HybridRankResult::NeedsOverlap(left, right) => {
                    let value = self.fragment_overlap(&left, &right)?;
                    overlaps.insert((left.id, right.id), value);
                }
            }
        }
    }

    pub(crate) fn diversify_candidates(
        &self,
        candidates: Vec<SearchCandidate>,
        limit: usize,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        if candidates.len() < 2 || limit == 0 {
            return Ok(candidates);
        }
        let mut overlaps = std::collections::HashMap::new();
        let mut ranker =
            reliquary_retrieval::HybridRanker::from_candidates(candidates, limit, usize::MAX);
        loop {
            match ranker.advance(&overlaps) {
                reliquary_retrieval::HybridRankResult::Complete(candidates) => {
                    return Ok(candidates);
                }
                reliquary_retrieval::HybridRankResult::NeedsOverlap(left, right) => {
                    let value = self.fragment_overlap(&left, &right)?;
                    overlaps.insert((left.id, right.id), value);
                }
            }
        }
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
