use std::collections::{BTreeMap, BTreeSet};

use reliquary_memory_core::{MemoryId, MemoryRef};

use crate::{MemoryLexicalHit, memory_retrieval::MemoryRetrievalHit};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemoryCandidateEvidence {
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemoryCandidate {
    pub memory_id: MemoryId,
    pub evidence: MemoryCandidateEvidence,
}

/// Forms the complete per-owner eligibility union without applying any result limit.
/// Output order is ascending canonical MemoryId, independent of input ordering.
pub fn union_memory_evidence(
    lexical: impl IntoIterator<Item = MemoryLexicalHit>,
    semantic: impl IntoIterator<Item = MemoryRetrievalHit>,
) -> Vec<MemoryCandidate> {
    let mut evidence = BTreeMap::<MemoryId, MemoryCandidateEvidence>::new();
    for hit in lexical {
        evidence.entry(hit.memory_id).or_default().lexical_score = Some(hit.score);
    }
    for hit in semantic {
        evidence.entry(hit.memory_id).or_default().semantic_score = Some(hit.score);
    }
    evidence
        .into_iter()
        .map(|(memory_id, evidence)| MemoryCandidate {
            memory_id,
            evidence,
        })
        .collect()
}

/// Owner-qualified candidate and its neutral-by-default Freshness grade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessCandidate {
    pub memory: MemoryRef,
    pub freshness: i16,
}

/// Orders all supplied candidates by descending Freshness, then canonical MemoryRef.
/// Grades affect order only; zero and negative values remain eligible.
pub fn order_by_freshness(candidates: &mut [FreshnessCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .freshness
            .cmp(&left.freshness)
            .then_with(|| left.memory.cmp(&right.memory))
    });
}

/// Full owner-qualified match set and the ordered candidates left after exclusions and limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessCandidateSelection {
    pub raw_matches: BTreeSet<MemoryRef>,
    pub candidates: Vec<FreshnessCandidate>,
}

/// Captures all raw matches, excludes already selected identities, then orders and limits.
/// Freshness grades affect order only; no grade value removes an otherwise eligible candidate.
pub fn limit_freshness_candidates(
    candidates: Vec<FreshnessCandidate>,
    selected: &BTreeSet<MemoryRef>,
    limit: usize,
) -> FreshnessCandidateSelection {
    let raw_matches = candidates
        .iter()
        .map(|candidate| candidate.memory.clone())
        .collect();
    let mut candidates = candidates
        .into_iter()
        .filter(|candidate| !selected.contains(&candidate.memory))
        .collect::<Vec<_>>();
    order_by_freshness(&mut candidates);
    candidates.truncate(limit);
    FreshnessCandidateSelection {
        raw_matches,
        candidates,
    }
}

/// One-hop neighbor set and query-state counts for an anchor report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticNeighborSelection {
    pub unseen: Vec<MemoryRef>,
    pub unseen_count: usize,
    pub declined_count: usize,
}

/// Deduplicates owner-qualified neighbors and classifies them against controller state.
/// Selected neighbors count in neither class; previously searched neighbors remain valid anchors.
pub fn select_agentic_neighbors(
    neighbors: impl IntoIterator<Item = MemoryRef>,
    searched: &BTreeSet<MemoryRef>,
    selected: &BTreeSet<MemoryRef>,
) -> AgenticNeighborSelection {
    let neighbors = neighbors.into_iter().collect::<BTreeSet<_>>();
    let declined_count = neighbors
        .iter()
        .filter(|memory| searched.contains(*memory) && !selected.contains(*memory))
        .count();
    let unseen = neighbors
        .into_iter()
        .filter(|memory| !searched.contains(memory) && !selected.contains(memory))
        .collect::<Vec<_>>();
    AgenticNeighborSelection {
        unseen_count: unseen.len(),
        unseen,
        declined_count,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FreshnessCandidate, MemoryCandidateEvidence, limit_freshness_candidates,
        order_by_freshness, select_agentic_neighbors, union_memory_evidence,
    };
    use crate::{MemoryLexicalHit, memory_retrieval::MemoryRetrievalHit};
    use reliquary_memory_core::{MemoryId, MemoryRef};
    use std::collections::BTreeSet;

    #[test]
    fn union_keeps_lexical_only_semantic_only_and_overlapping_candidates() {
        let lexical = vec![
            MemoryLexicalHit {
                memory_id: MemoryId([2; 32]),
                score: 0.6,
            },
            MemoryLexicalHit {
                memory_id: MemoryId([1; 32]),
                score: 0.4,
            },
        ];
        let semantic = vec![
            MemoryRetrievalHit {
                memory_id: MemoryId([2; 32]),
                score: 0.9,
            },
            MemoryRetrievalHit {
                memory_id: MemoryId([3; 32]),
                score: 0.8,
            },
        ];

        assert_eq!(
            union_memory_evidence(lexical, semantic),
            vec![
                super::MemoryCandidate {
                    memory_id: MemoryId([1; 32]),
                    evidence: MemoryCandidateEvidence {
                        lexical_score: Some(0.4),
                        semantic_score: None,
                    },
                },
                super::MemoryCandidate {
                    memory_id: MemoryId([2; 32]),
                    evidence: MemoryCandidateEvidence {
                        lexical_score: Some(0.6),
                        semantic_score: Some(0.9),
                    },
                },
                super::MemoryCandidate {
                    memory_id: MemoryId([3; 32]),
                    evidence: MemoryCandidateEvidence {
                        lexical_score: None,
                        semantic_score: Some(0.8),
                    },
                },
            ]
        );
    }

    #[test]
    fn freshness_sort_uses_owner_qualified_identity_and_keeps_neutral_grades() {
        let same_id = MemoryId([1; 32]);
        let mut candidates = vec![
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "z-owner".into(),
                    memory_id: same_id,
                },
                freshness: 0,
            },
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "b-owner".into(),
                    memory_id: MemoryId([2; 32]),
                },
                freshness: 10,
            },
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "a-owner".into(),
                    memory_id: same_id,
                },
                freshness: 0,
            },
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "c-owner".into(),
                    memory_id: same_id,
                },
                freshness: -3,
            },
        ];

        order_by_freshness(&mut candidates);

        assert_eq!(candidates[0].memory.owner_id, "b-owner");
        assert_eq!(candidates[1].memory.owner_id, "a-owner");
        assert_eq!(candidates[2].memory.owner_id, "z-owner");
        assert_eq!(candidates[3].memory.owner_id, "c-owner");
    }

    #[test]
    fn freshness_limit_captures_full_raw_set_before_selected_exclusion() {
        let selected = MemoryRef {
            owner_id: "owner-a".into(),
            memory_id: MemoryId([1; 32]),
        };
        let candidates = vec![
            FreshnessCandidate {
                memory: selected.clone(),
                freshness: 100,
            },
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "owner-b".into(),
                    memory_id: MemoryId([1; 32]),
                },
                freshness: 0,
            },
            FreshnessCandidate {
                memory: MemoryRef {
                    owner_id: "owner-c".into(),
                    memory_id: MemoryId([2; 32]),
                },
                freshness: -1,
            },
        ];

        let selection =
            limit_freshness_candidates(candidates, &BTreeSet::from([selected.clone()]), 1);

        assert_eq!(selection.raw_matches.len(), 3);
        assert!(selection.raw_matches.contains(&selected));
        assert_eq!(selection.candidates.len(), 1);
        assert_eq!(selection.candidates[0].memory.owner_id, "owner-b");
    }

    #[test]
    fn neighbor_selection_deduplicates_and_preserves_controller_meanings() {
        let unseen = MemoryRef {
            owner_id: "owner".into(),
            memory_id: MemoryId([1; 32]),
        };
        let declined = MemoryRef {
            owner_id: "owner".into(),
            memory_id: MemoryId([2; 32]),
        };
        let already_selected = MemoryRef {
            owner_id: "owner".into(),
            memory_id: MemoryId([3; 32]),
        };
        let other_owner_same_id = MemoryRef {
            owner_id: "other".into(),
            memory_id: MemoryId([1; 32]),
        };
        let searched = BTreeSet::from([declined.clone(), already_selected.clone()]);
        let selected = BTreeSet::from([already_selected.clone()]);

        let selection = select_agentic_neighbors(
            vec![
                unseen.clone(),
                unseen.clone(),
                declined.clone(),
                already_selected.clone(),
                other_owner_same_id.clone(),
            ],
            &searched,
            &selected,
        );

        assert_eq!(selection.unseen_count, 2);
        assert_eq!(selection.declined_count, 1);
        assert_eq!(selection.unseen, vec![other_owner_same_id, unseen]);
    }
}
