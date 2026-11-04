use super::*;
use crate::{rank_lanes, select_candidates};

fn candidate(index: u64) -> RankedCandidate {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&index.to_be_bytes());
    // Deliberately create ties and sparse, individually qualifying lanes.
    let semantic = match index % 11 {
        0 => None,
        1 => Some(0.0),
        _ => Some((index % 31) as f64 / 32.0),
    };
    RankedCandidate {
        id: MemoryId(id),
        source_timestamp_ns: (index % 4 != 0).then_some((index as i64 % 700) - 150),
        semantic_score: semantic,
        lexical_score: (index % 17) as f64 / 20.0,
        temporal_score: (index % 13) as f64 / 20.0,
        semantic_rank: None,
        prior_rank: None,
        lexical_rank: None,
        temporal_rank: None,
        fused_score: 0.0,
    }
}

#[test]
fn bounded_frontier_is_identical_to_full_ranking_over_large_candidate_sets() {
    let configs = [
        DreamCandidateConfig::default(),
        DreamCandidateConfig {
            limit: 2,
            semantic_limit: 1,
            prior_semantic_quota: 2,
            lexical_limit: 1,
            temporal_limit: 1,
        },
        DreamCandidateConfig {
            limit: 16,
            semantic_limit: 0,
            prior_semantic_quota: 0,
            lexical_limit: 8,
            temporal_limit: 8,
        },
        DreamCandidateConfig {
            limit: 12,
            semantic_limit: 30,
            prior_semantic_quota: 3,
            lexical_limit: 0,
            temporal_limit: 0,
        },
    ];
    for source_time in [None, Some(200)] {
        for config in configs {
            let mut full: Vec<_> = (0..3000).map(candidate).collect();
            let mut bounded = DreamCandidateFrontier::new(source_time, config);
            for item in &full {
                bounded.push(item.clone());
                assert!(
                    bounded.retained_entries()
                        <= config.semantic_limit
                            + config.prior_semantic_quota
                            + config.lexical_limit
                            + config.temporal_limit
                );
            }
            rank_lanes(source_time, &mut full, config);
            let expected = select_candidates(full, config);
            let actual = bounded.finish();
            assert_eq!(
                actual.iter().map(|item| item.id).collect::<Vec<_>>(),
                expected.iter().map(|item| item.id).collect::<Vec<_>>(),
                "source_time={source_time:?}, config={config:?}",
            );
            for (actual, expected) in actual.iter().zip(&expected) {
                assert_eq!(
                    actual.fused_score, expected.fused_score,
                    "rank scores must be unchanged"
                );
            }
        }
    }
}

#[test]
fn prior_candidates_outside_semantic_limit_remain_reserved() {
    let config = DreamCandidateConfig {
        limit: 2,
        semantic_limit: 1,
        prior_semantic_quota: 1,
        lexical_limit: 0,
        temporal_limit: 0,
    };
    let mut newest = candidate(1);
    newest.semantic_score = Some(0.99);
    newest.source_timestamp_ns = Some(150);
    let mut older = candidate(2);
    older.semantic_score = Some(0.3);
    older.source_timestamp_ns = Some(50);
    let mut frontier = DreamCandidateFrontier::new(Some(100), config);
    frontier.push(newest.clone());
    frontier.push(older.clone());
    let selected = frontier.finish();
    assert_eq!(selected.len(), 2);
    assert!(
        selected
            .iter()
            .any(|item| item.id == older.id && item.prior_rank == Some(1))
    );
    assert!(
        selected
            .iter()
            .any(|item| item.id == newest.id && item.semantic_rank == Some(1))
    );
}
