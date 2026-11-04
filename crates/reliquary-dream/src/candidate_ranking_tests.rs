use super::*;

fn record() -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([1; 32]),
        revision: 1,
        body_id: reliquary_memory_core::MemoryBodyId([2; 32]),
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: Some(MemoryId([3; 32])),
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: "one".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        memory_version: 1,
    }
}

#[test]
fn lexical_lane_matches_reference_token_and_metadata_weights() {
    let source = record();
    let mut other = source.clone();
    let first = MemoryBody {
        title: "Alpha beta".into(),
        content: "beta x".into(),
    };
    let second = MemoryBody {
        title: "BETA gamma".into(),
        content: "gamma".into(),
    };
    assert!((lexical_score(&source, &first, &other, &second) - (1.0 / 3.0 + 0.28)).abs() < 1e-12);
    other.parent_id = None;
    other.category = "preference".into();
    other.memory_type = "communication".into();
    assert!((lexical_score(&source, &first, &other, &second) - 1.0 / 3.0).abs() < 1e-12);
}

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
