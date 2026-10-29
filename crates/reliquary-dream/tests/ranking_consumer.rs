use reliquary_dream::{
    DreamCandidateConfig, RankedCandidate, candidate_cosine, rank_lanes, select_candidates,
};
use reliquary_memory_core::MemoryId;

fn candidate(
    id: u8,
    timestamp: i64,
    semantic: f64,
    lexical: f64,
    temporal: f64,
) -> RankedCandidate {
    RankedCandidate {
        id: MemoryId([id; 32]),
        source_timestamp_ns: Some(timestamp),
        semantic_score: Some(semantic),
        lexical_score: lexical,
        semantic_rank: None,
        prior_rank: None,
        lexical_rank: None,
        temporal_score: temporal,
        temporal_rank: None,
        fused_score: 0.0,
    }
}
#[test]
fn lanes_reserve_prior_candidates_and_break_ties_by_id() {
    let config = DreamCandidateConfig {
        limit: 2,
        semantic_limit: 3,
        prior_semantic_quota: 1,
        lexical_limit: 3,
        temporal_limit: 3,
    };
    let mut candidates = vec![
        candidate(3, 5, 0.8, 1.0, 0.0),
        candidate(2, 20, 0.9, 1.0, 1.0),
        candidate(1, 30, 0.9, 1.0, 1.0),
    ];
    rank_lanes(Some(10), &mut candidates, config);
    let selected = select_candidates(candidates, config);
    assert_eq!(
        selected.iter().map(|c| c.id.0[0]).collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(candidate_cosine(&[1.0, 0.0], &[1.0, 0.0]), Ok(1.0));
    assert_eq!(candidate_cosine(&[0.0], &[1.0]), Err("zero vector"));
}
