//! Pure planning primitives for the first historical Freshness import.
//!
//! This module deliberately has no storage or scoring side effects.  Step 15
//! wires its clock proof to Archive; later steps consume the plan to emit and
//! score historical events.

use std::collections::{BTreeMap, BTreeSet};

pub const HISTORICAL_PLANNER_VERSION: u16 = 1;
pub use super::historical_contract::HISTORICAL_POLICY_DIGEST;

pub use crate::historical_timeline::{
    HistoricalSource, HistoricalTimeline, PlannerError, SourceKey, TimelineEntry,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeEvidence {
    pub id: String,
    pub path: Vec<SourceKey>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEvidence {
    pub memory_id: String,
    pub source: Option<SourceKey>,
    pub content: Option<SourceKey>,
    pub grounding: Option<SourceKey>,
    pub episode_id: Option<String>,
    pub cohort_id: Option<String>,
    pub archived: bool,
    pub superseded: bool,
    pub provisional_replaceable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAvailability {
    Supported { position: u64 },
    Unsupported,
    CreditExcluded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryPlan {
    pub memory_id: String,
    pub availability: MemoryAvailability,
    pub cohort_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalProvenancePlan {
    pub memories: Vec<MemoryPlan>,
    pub coverage: BTreeMap<String, String>,
}

impl HistoricalProvenancePlan {
    pub fn plan(
        timeline: &HistoricalTimeline,
        episodes: &[EpisodeEvidence],
        memories: &[MemoryEvidence],
    ) -> Result<Self, PlannerError> {
        let episode_map = episodes
            .iter()
            .map(|e| (e.id.as_str(), e))
            .collect::<BTreeMap<_, _>>();
        let mut plans = Vec::with_capacity(memories.len());
        let mut coverage = BTreeMap::new();
        for memory in memories {
            if memory.archived || memory.superseded {
                plans.push(MemoryPlan {
                    memory_id: memory.memory_id.clone(),
                    availability: MemoryAvailability::CreditExcluded,
                    cohort_id: memory
                        .cohort_id
                        .clone()
                        .unwrap_or_else(|| memory.memory_id.clone()),
                });
                coverage.insert(memory.memory_id.clone(), "lifecycle-excluded".into());
                continue;
            }
            let mut evidence = Vec::new();
            for key in [
                memory.source.as_ref(),
                memory.content.as_ref(),
                memory.grounding.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                evidence.push(key);
            }
            if let Some(id) = &memory.episode_id {
                let episode =
                    episode_map
                        .get(id.as_str())
                        .ok_or_else(|| PlannerError::UnknownEpisode {
                            memory_id: memory.memory_id.clone(),
                        })?;
                if !episode.complete {
                    coverage.insert(memory.memory_id.clone(), "episode-incomplete".into());
                    plans.push(MemoryPlan {
                        memory_id: memory.memory_id.clone(),
                        availability: MemoryAvailability::Unsupported,
                        cohort_id: memory.cohort_id.clone().unwrap_or_else(|| id.clone()),
                    });
                    continue;
                }
                evidence.extend(episode.path.iter());
            }
            if evidence.is_empty() {
                coverage.insert(memory.memory_id.clone(), "missing-provenance".into());
                plans.push(MemoryPlan {
                    memory_id: memory.memory_id.clone(),
                    availability: MemoryAvailability::Unsupported,
                    cohort_id: memory
                        .cohort_id
                        .clone()
                        .or_else(|| memory.episode_id.clone())
                        .unwrap_or_else(|| memory.memory_id.clone()),
                });
                continue;
            }
            let mut max_position = 0_u64;
            for key in evidence {
                let Some(position) = timeline.position(key) else {
                    coverage.insert(
                        memory.memory_id.clone(),
                        format!("missing-evidence:{}", key.node_id),
                    );
                    plans.push(MemoryPlan {
                        memory_id: memory.memory_id.clone(),
                        availability: MemoryAvailability::Unsupported,
                        cohort_id: memory
                            .cohort_id
                            .clone()
                            .or_else(|| memory.episode_id.clone())
                            .unwrap_or_else(|| memory.memory_id.clone()),
                    });
                    continue;
                };
                max_position = max_position.max(position);
            }
            let cohort_id = memory
                .cohort_id
                .clone()
                .or_else(|| memory.episode_id.clone())
                .unwrap_or_else(|| memory.memory_id.clone());
            if memory.provisional_replaceable == false && max_position > 0 {
                return Err(PlannerError::UnsafeProvisionalReplacement(
                    memory.memory_id.clone(),
                ));
            }
            plans.push(MemoryPlan {
                memory_id: memory.memory_id.clone(),
                availability: MemoryAvailability::Supported {
                    position: max_position.max(1),
                },
                cohort_id,
            });
            coverage.insert(memory.memory_id.clone(), "supported".into());
        }
        let mut cohorts = BTreeMap::<String, u64>::new();
        for plan in &plans {
            if let MemoryAvailability::Supported { position } = plan.availability {
                if let Some(previous) = cohorts.insert(plan.cohort_id.clone(), position) {
                    if previous != position {
                        return Err(PlannerError::ConflictingCohort(plan.cohort_id.clone()));
                    }
                }
            }
        }
        Ok(Self {
            memories: plans,
            coverage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn src(c: &str, n: &str, p: Option<&str>, role: &str, t: i64) -> HistoricalSource {
        HistoricalSource {
            key: SourceKey {
                conversation_id: c.into(),
                node_id: n.into(),
            },
            parent_id: p.map(str::to_owned),
            role: role.into(),
            timestamp_ns: t,
        }
    }
    #[test]
    fn historical_accepted_clock_rejects_missing_duplicate_gap_foreign_and_reversed_ancestry() {
        let input = vec![
            src("c", "u", None, "user", 20),
            src("c", "a", Some("u"), "assistant", 1),
        ];
        let u = input[0].key.clone();
        let a = input[1].key.clone();
        for (positions, count) in [
            (BTreeMap::from([(u.clone(), 1)]), 2),
            (BTreeMap::from([(u.clone(), 1), (a.clone(), 1)]), 2),
            (BTreeMap::from([(u.clone(), 1), (a.clone(), 3)]), 2),
            (BTreeMap::from([(u.clone(), 2), (a.clone(), 1)]), 2),
            (BTreeMap::from([(u.clone(), 1), (a.clone(), 2)]), 3),
            (
                BTreeMap::from([
                    (u.clone(), 1),
                    (
                        SourceKey {
                            conversation_id: "foreign".into(),
                            node_id: "a".into(),
                        },
                        2,
                    ),
                ]),
                2,
            ),
        ] {
            assert!(
                HistoricalTimeline::from_accepted_archive(input.clone(), &positions, count)
                    .is_err()
            );
        }
        let valid = BTreeMap::from([(u, 1), (a, 2)]);
        assert!(HistoricalTimeline::from_accepted_archive(input.clone(), &valid, 2).is_ok());
        let mut broken = input.clone();
        broken[1].parent_id = Some("missing".into());
        assert!(matches!(
            HistoricalTimeline::from_accepted_archive(broken, &valid, 2),
            Err(PlannerError::MissingParent(_))
        ));
        let mut cycle = input.clone();
        cycle[0].parent_id = Some("a".into());
        assert!(matches!(
            HistoricalTimeline::from_accepted_archive(cycle, &valid, 2),
            Err(PlannerError::Cycle)
        ));
        let mut conflict = input.clone();
        conflict.push(src("c", "u", None, "user", 99));
        assert!(matches!(
            HistoricalTimeline::from_accepted_archive(conflict, &valid, 2),
            Err(PlannerError::ConflictingDuplicate(_))
        ));
    }
    #[test]
    fn historical_non_activity_ancestry_cycle_rejects_before_projection() {
        for activity in [false, true] {
            let mut sources = vec![
                src("c", "s", Some("t"), "system", 0),
                src("c", "t", Some("s"), "tool", 0),
            ];
            if activity {
                sources.push(src("c", "u", Some("s"), "user", 1));
            }
            assert_eq!(HistoricalTimeline::plan(sources), Err(PlannerError::Cycle));
        }
    }
    #[test]
    fn ties_are_utf8_stable_and_ancestry_wins() {
        let timeline = HistoricalTimeline::plan(vec![
            src("b", "z", None, "user", 1),
            src("a", "y", None, "assistant", 1),
            src("a", "a", Some("y"), "assistant", 2),
        ])
        .unwrap();
        assert_eq!(
            timeline
                .activity_order
                .iter()
                .map(|k| (&k.conversation_id, &k.node_id))
                .collect::<Vec<_>>(),
            vec![
                (&"a".to_string(), &"y".to_string()),
                (&"b".to_string(), &"z".to_string()),
                (&"a".to_string(), &"a".to_string())
            ]
        );
    }
    #[test]
    fn missing_parent_and_clock_mismatch_are_rejected() {
        let err =
            HistoricalTimeline::plan(vec![src("c", "n", Some("missing"), "user", 1)]).unwrap_err();
        assert!(matches!(err, PlannerError::MissingParent(_)));
        let timeline = HistoricalTimeline::plan(vec![src("c", "n", None, "user", 1)]).unwrap();
        let mut positions = BTreeMap::new();
        positions.insert(
            SourceKey {
                conversation_id: "c".into(),
                node_id: "n".into(),
            },
            2,
        );
        assert!(matches!(
            timeline.validate_archive_clock(&positions, 1),
            Err(PlannerError::ArchiveClockMismatch { .. })
        ));
    }
    #[test]
    fn non_activity_ancestry_bridges_and_provenance_reports_coverage() {
        let timeline = HistoricalTimeline::plan(vec![
            src("c", "s", None, "system", 0),
            src("c", "u", Some("s"), "user", 1),
            src("c", "t", Some("u"), "tool", 2),
            src("c", "a", Some("t"), "assistant", 3),
        ])
        .unwrap();
        assert_eq!(timeline.activity_order.len(), 2);
        let plan = HistoricalProvenancePlan::plan(
            &timeline,
            &[],
            &[MemoryEvidence {
                memory_id: "m".into(),
                source: Some(SourceKey {
                    conversation_id: "c".into(),
                    node_id: "u".into(),
                }),
                content: None,
                grounding: None,
                episode_id: None,
                cohort_id: None,
                archived: false,
                superseded: false,
                provisional_replaceable: true,
            }],
        )
        .unwrap();
        assert_eq!(
            plan.memories[0].availability,
            MemoryAvailability::Supported { position: 1 }
        );
    }
}
