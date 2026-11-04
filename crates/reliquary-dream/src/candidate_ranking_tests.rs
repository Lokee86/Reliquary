use super::*;

fn candidate(
    id: u8,
    time: Option<i64>,
    semantic: Option<f64>,
    lexical: f64,
    temporal: f64,
) -> RankedCandidate {
    RankedCandidate {
        id: MemoryId([id; 32]),
        source_timestamp_ns: time,
        semantic_score: semantic,
        lexical_score: lexical,
        temporal_score: temporal,
        semantic_rank: None,
        prior_rank: None,
        lexical_rank: None,
        temporal_rank: None,
        fused_score: 0.0,
    }
}

#[test]
fn fused_lanes_preserve_prior_quota_and_reference_tie_order() {
    let config = DreamCandidateConfig {
        limit: 2,
        prior_semantic_quota: 1,
        ..Default::default()
    };
    assert!(validate_candidate_config(config).is_ok());
    let mut candidates = vec![
        candidate(3, None, None, 1.0, 1.0),
        candidate(2, Some(20), Some(0.9), 0.0, 0.0),
        candidate(1, Some(1), Some(0.4), 0.0, 0.0),
    ];
    rank_lanes(Some(10), &mut candidates, config);
    let selected = select_candidates(candidates, config);
    assert_eq!(
        selected.iter().map(|c| c.id).collect::<Vec<_>>(),
        [MemoryId([1; 32]), MemoryId([3; 32])]
    );
    assert_eq!(selected[0].prior_rank, Some(1));
    let mut tied = vec![
        candidate(2, None, Some(0.5), 0.0, 0.0),
        candidate(1, None, Some(0.5), 0.0, 0.0),
    ];
    rank_lanes(None, &mut tied, config);
    assert_eq!(select_candidates(tied, config)[0].id, MemoryId([1; 32]));
}

#[test]
fn reranking_clears_old_evidence_and_rejects_nonfinite_vectors() {
    let config = DreamCandidateConfig::default();
    let mut candidates = vec![candidate(1, Some(1), Some(0.5), 0.0, 0.0)];
    rank_lanes(Some(2), &mut candidates, config);
    candidates[0].semantic_score = Some(f64::NAN);
    rank_lanes(Some(2), &mut candidates, config);
    assert!(select_candidates(candidates, config).is_empty());
    assert!(candidate_cosine(&[f32::NAN], &[1.0]).is_err());
    assert!(candidate_cosine(&[0.0], &[1.0]).is_err());
    assert_eq!(candidate_cosine(&[1.0, 0.0], &[1.0, 0.0]).unwrap(), 1.0);
    assert!(validate_candidate_config(DreamCandidateConfig { limit: 0, ..config }).is_err());
}
