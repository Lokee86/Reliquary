use crate::{DreamCandidate, DreamCandidateConfig, DreamMemoryContext};

#[derive(Clone)]
pub(crate) struct ScoredCandidate {
    pub(crate) context: DreamMemoryContext,
    pub(crate) semantic_score: Option<f64>,
    pub(crate) lexical_score: f64,
    pub(crate) semantic_rank: Option<usize>,
    pub(crate) prior_rank: Option<usize>,
    pub(crate) lexical_rank: Option<usize>,
    pub(crate) temporal_score: f64,
    pub(crate) temporal_rank: Option<usize>,
    pub(crate) temporal_matches: Vec<crate::DreamTemporalMatch>,
    pub(crate) fused_score: f64,
}

pub(crate) fn rank_and_select(
    source: &DreamMemoryContext,
    candidates: Vec<ScoredCandidate>,
    config: DreamCandidateConfig,
) -> Vec<DreamCandidate> {
    let mut contexts: std::collections::HashMap<_, _> = candidates
        .into_iter()
        .map(|candidate| (candidate.context.memory.id, candidate))
        .collect();
    let mut ranked: Vec<_> = contexts
        .values()
        .map(|candidate| reliquary_dream::RankedCandidate {
            id: candidate.context.memory.id,
            source_timestamp_ns: candidate.context.source_timestamp_ns,
            semantic_score: candidate.semantic_score,
            lexical_score: candidate.lexical_score,
            semantic_rank: candidate.semantic_rank,
            prior_rank: candidate.prior_rank,
            lexical_rank: candidate.lexical_rank,
            temporal_score: candidate.temporal_score,
            temporal_rank: candidate.temporal_rank,
            fused_score: candidate.fused_score,
        })
        .collect();
    reliquary_dream::rank_lanes(source.source_timestamp_ns, &mut ranked, config);
    reliquary_dream::select_candidates(ranked, config)
        .into_iter()
        .map(|ranked| {
            let mut candidate = contexts
                .remove(&ranked.id)
                .expect("ranked candidate was supplied");
            candidate.semantic_rank = ranked.semantic_rank;
            candidate.prior_rank = ranked.prior_rank;
            candidate.lexical_rank = ranked.lexical_rank;
            candidate.temporal_rank = ranked.temporal_rank;
            to_public_candidate(candidate)
        })
        .collect()
}

fn to_public_candidate(candidate: ScoredCandidate) -> DreamCandidate {
    DreamCandidate {
        context: candidate.context,
        semantic_score: candidate.semantic_score,
        semantic_rank: candidate.semantic_rank,
        prior_semantic_rank: candidate.prior_rank,
        lexical_score: candidate.lexical_score,
        lexical_rank: candidate.lexical_rank,
        temporal_score: candidate.temporal_score,
        temporal_rank: candidate.temporal_rank,
        temporal_matches: candidate.temporal_matches,
    }
}
