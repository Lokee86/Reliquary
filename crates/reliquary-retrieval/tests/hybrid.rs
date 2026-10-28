use reliquary_archive::{Fragment, FragmentId};
use reliquary_retrieval::{HybridRankResult, HybridRanker, LexicalHit, RetrievalConfig};
use std::collections::HashMap;

fn fragment(id: u8, start: &str) -> Fragment {
    Fragment {
        id: FragmentId([id; 32]),
        conversation_id: "same".into(),
        start_node_id: start.into(),
        end_node_id: start.into(),
    }
}

#[test]
fn hybrid_resumes_with_supplied_overlap_and_deduplicates_ranges() {
    let lexical = vec![
        LexicalHit {
            fragment: fragment(1, "a"),
            score: 1.0,
        },
        LexicalHit {
            fragment: fragment(2, "b"),
            score: 0.9,
        },
        LexicalHit {
            fragment: fragment(3, "c"),
            score: 0.8,
        },
        LexicalHit {
            fragment: fragment(4, "a"),
            score: 0.7,
        },
    ];
    let mut ranker = HybridRanker::new(&lexical, &[], RetrievalConfig::default(), (0.45, 0.55));
    let mut overlaps = HashMap::new();
    let mut requests = 0;
    loop {
        match ranker.advance(&overlaps) {
            HybridRankResult::NeedsOverlap(left, right) => {
                requests += 1;
                let value = if left.id == FragmentId([2; 32]) && right.id == FragmentId([1; 32]) {
                    1.0
                } else {
                    0.0
                };
                overlaps.insert((left.id, right.id), value);
            }
            HybridRankResult::Complete(hits) => {
                assert_eq!(
                    hits.iter().map(|hit| hit.fragment.id).collect::<Vec<_>>(),
                    vec![
                        FragmentId([1; 32]),
                        FragmentId([3; 32]),
                        FragmentId([2; 32])
                    ]
                );
                assert_eq!(hits[0].combined_score, 1.0);
                assert!(requests > 0);
                break;
            }
        }
    }
}
