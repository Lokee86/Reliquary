//! Pure planning primitives for the first historical Freshness import.
//!
//! This module deliberately has no storage or scoring side effects.  Step 15
//! wires its clock proof to Archive; later steps consume the plan to emit and
//! score historical events.

use std::collections::{BTreeMap, BTreeSet};

pub const HISTORICAL_PLANNER_VERSION: u16 = 1;
pub const HISTORICAL_POLICY_DIGEST: [u8; 32] = [
    0x0c, 0xad, 0x2e, 0x7f, 0xd7, 0x63, 0xde, 0x37, 0x75, 0x3a, 0x54, 0xc2, 0x8d, 0xf1, 0xc7, 0xb6,
    0x30, 0x3c, 0x6c, 0x81, 0x81, 0x45, 0x49, 0x2c, 0xa4, 0xa7, 0x09, 0x02, 0x3c, 0x42, 0x09, 0x08,
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceKey {
    pub conversation_id: String,
    pub node_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalSource {
    pub key: SourceKey,
    pub parent_id: Option<String>,
    pub role: String,
    pub timestamp_ns: i64,
}

impl HistoricalSource {
    pub fn activity(&self) -> bool {
        matches!(self.role.as_str(), "user" | "assistant")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    pub key: SourceKey,
    pub parent_id: Option<String>,
    pub timestamp_ns: i64,
    pub activity: bool,
    pub position: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalTimeline {
    pub entries: Vec<TimelineEntry>,
    pub activity_order: Vec<SourceKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlannerError {
    ConflictingDuplicate(SourceKey),
    MissingParent(SourceKey),
    CrossConversationParent(SourceKey),
    Cycle,
    TimestampRegression {
        ancestor: SourceKey,
        descendant: SourceKey,
    },
    ArchiveClockMismatch {
        key: SourceKey,
        expected: u64,
        actual: Option<u64>,
    },
    ArchiveCountMismatch {
        expected: u64,
        actual: u64,
    },
    MissingEvidence {
        memory_id: String,
        key: SourceKey,
    },
    UnknownEpisode {
        memory_id: String,
    },
    ConflictingCohort(String),
    UnsafeProvisionalReplacement(String),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingDuplicate(k) => write!(
                f,
                "conflicting duplicate source {}/{}",
                k.conversation_id, k.node_id
            ),
            Self::MissingParent(k) => {
                write!(f, "missing parent for {}/{}", k.conversation_id, k.node_id)
            }
            Self::CrossConversationParent(k) => write!(
                f,
                "cross-conversation parent for {}/{}",
                k.conversation_id, k.node_id
            ),
            Self::Cycle => f.write_str("source ancestry contains a cycle"),
            Self::TimestampRegression {
                ancestor,
                descendant,
            } => write!(
                f,
                "timestamp regression from {} to {}",
                ancestor.node_id, descendant.node_id
            ),
            Self::ArchiveClockMismatch {
                key,
                expected,
                actual,
            } => write!(
                f,
                "archive clock mismatch for {}: expected {}, got {:?}",
                key.node_id, expected, actual
            ),
            Self::ArchiveCountMismatch { expected, actual } => write!(
                f,
                "archive count mismatch: expected {}, got {}",
                expected, actual
            ),
            Self::MissingEvidence { memory_id, key } => write!(
                f,
                "missing evidence {} for memory {}",
                key.node_id, memory_id
            ),
            Self::UnknownEpisode { memory_id } => {
                write!(f, "unknown episode for memory {}", memory_id)
            }
            Self::ConflictingCohort(id) => write!(f, "conflicting birth cohort {}", id),
            Self::UnsafeProvisionalReplacement(id) => {
                write!(f, "unsafe provisional replacement for memory {}", id)
            }
        }
    }
}
impl std::error::Error for PlannerError {}

impl HistoricalTimeline {
    pub fn plan(input: Vec<HistoricalSource>) -> Result<Self, PlannerError> {
        let mut all = BTreeMap::<SourceKey, HistoricalSource>::new();
        for source in input {
            if let Some(previous) = all.get(&source.key) {
                if previous != &source {
                    return Err(PlannerError::ConflictingDuplicate(source.key));
                }
            } else {
                all.insert(source.key.clone(), source);
            }
        }
        for source in all.values() {
            if let Some(parent) = &source.parent_id {
                let parent_key = SourceKey {
                    conversation_id: source.key.conversation_id.clone(),
                    node_id: parent.clone(),
                };
                if !all.contains_key(&parent_key) {
                    return Err(PlannerError::MissingParent(source.key.clone()));
                }
            }
        }
        let mut validated = BTreeSet::new();
        for key in all.keys() {
            let mut chain = BTreeSet::new();
            let mut cursor = Some(key.clone());
            while let Some(key) = cursor {
                if validated.contains(&key) {
                    break;
                }
                if !chain.insert(key.clone()) {
                    return Err(PlannerError::Cycle);
                }
                cursor = all[&key].parent_id.as_ref().map(|parent| SourceKey {
                    conversation_id: key.conversation_id.clone(),
                    node_id: parent.clone(),
                });
            }
            validated.extend(chain);
        }
        let mut indegree = BTreeMap::<SourceKey, usize>::new();
        let mut children = BTreeMap::<SourceKey, Vec<SourceKey>>::new();
        for source in all.values().filter(|s| s.activity()) {
            indegree.entry(source.key.clone()).or_insert(0);
            let mut cursor = source.parent_id.clone();
            while let Some(parent_id) = cursor {
                let parent = all
                    .get(&SourceKey {
                        conversation_id: source.key.conversation_id.clone(),
                        node_id: parent_id.clone(),
                    })
                    .expect("validated parent");
                if parent.activity() {
                    *indegree.entry(source.key.clone()).or_insert(0) += 1;
                    children
                        .entry(parent.key.clone())
                        .or_default()
                        .push(source.key.clone());
                    if parent.timestamp_ns > source.timestamp_ns {
                        return Err(PlannerError::TimestampRegression {
                            ancestor: parent.key.clone(),
                            descendant: source.key.clone(),
                        });
                    }
                    break;
                }
                cursor = parent.parent_id.clone();
            }
        }
        let mut ready: BTreeSet<(i64, SourceKey)> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| (all[k].timestamp_ns, k.clone()))
            .collect();
        let mut order = Vec::with_capacity(indegree.len());
        while let Some((_, key)) = ready.pop_first() {
            order.push(key.clone());
            if let Some(next) = children.get(&key) {
                for child in next {
                    let d = indegree.get_mut(child).expect("child exists");
                    *d -= 1;
                    if *d == 0 {
                        ready.insert((all[child].timestamp_ns, child.clone()));
                    }
                }
            }
        }
        if order.len() != indegree.len() {
            return Err(PlannerError::Cycle);
        }
        let positions = order
            .iter()
            .enumerate()
            .map(|(i, k)| (k, (i + 1) as u64))
            .collect::<BTreeMap<_, _>>();
        let entries = all
            .values()
            .map(|s| TimelineEntry {
                key: s.key.clone(),
                parent_id: s.parent_id.clone(),
                timestamp_ns: s.timestamp_ns,
                activity: s.activity(),
                position: positions.get(&s.key).copied(),
            })
            .collect();
        Ok(Self {
            entries,
            activity_order: order,
        })
    }

    pub fn validate_archive_clock(
        &self,
        positions: &BTreeMap<SourceKey, u64>,
        count: u64,
    ) -> Result<(), PlannerError> {
        if count != self.activity_order.len() as u64 {
            return Err(PlannerError::ArchiveCountMismatch {
                expected: self.activity_order.len() as u64,
                actual: count,
            });
        }
        for (i, key) in self.activity_order.iter().enumerate() {
            let expected = (i + 1) as u64;
            if positions.get(key).copied() != Some(expected) {
                return Err(PlannerError::ArchiveClockMismatch {
                    key: key.clone(),
                    expected,
                    actual: positions.get(key).copied(),
                });
            }
        }
        Ok(())
    }

    pub fn position(&self, key: &SourceKey) -> Option<u64> {
        self.activity_order
            .iter()
            .position(|candidate| candidate == key)
            .map(|i| (i + 1) as u64)
    }
}

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
