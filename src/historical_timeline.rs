//! Immutable source timeline facts and accepted Archive clock validation.

use std::collections::{BTreeMap, BTreeSet};

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
    ArchiveAncestryMismatch {
        ancestor: SourceKey,
        descendant: SourceKey,
    },
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
            Self::ArchiveAncestryMismatch {
                ancestor,
                descendant,
            } => write!(
                f,
                "accepted ancestry order mismatch from {} to {}",
                ancestor.node_id, descendant.node_id
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

fn validated_sources(
    input: Vec<HistoricalSource>,
) -> Result<BTreeMap<SourceKey, HistoricalSource>, PlannerError> {
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
    Ok(all)
}

impl HistoricalTimeline {
    pub fn plan(input: Vec<HistoricalSource>) -> Result<Self, PlannerError> {
        let all = validated_sources(input)?;
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

    /// Existing RELs use Archive positions, never a second timestamp order.
    pub fn from_accepted_archive(
        input: Vec<HistoricalSource>,
        positions: &BTreeMap<SourceKey, u64>,
        count: u64,
    ) -> Result<Self, PlannerError> {
        let all = validated_sources(input)?;
        let activity_count = all.values().filter(|s| s.activity()).count() as u64;
        if count != activity_count || positions.len() as u64 != activity_count {
            return Err(PlannerError::ArchiveCountMismatch {
                expected: activity_count,
                actual: if count != activity_count {
                    count
                } else {
                    positions.len() as u64
                },
            });
        }
        let mut order = Vec::new();
        for source in all.values().filter(|s| s.activity()) {
            let position = positions.get(&source.key).copied().ok_or_else(|| {
                PlannerError::ArchiveClockMismatch {
                    key: source.key.clone(),
                    expected: 1,
                    actual: None,
                }
            })?;
            order.push((position, source.key.clone()));
        }
        order.sort();
        for (index, (position, key)) in order.iter().enumerate() {
            if *position != index as u64 + 1 {
                return Err(PlannerError::ArchiveClockMismatch {
                    key: key.clone(),
                    expected: index as u64 + 1,
                    actual: Some(*position),
                });
            }
            let mut parent = all[key].parent_id.clone();
            while let Some(id) = parent {
                let ancestor = &all[&SourceKey {
                    conversation_id: key.conversation_id.clone(),
                    node_id: id,
                }];
                if ancestor.activity() {
                    if positions[&ancestor.key] >= *position {
                        return Err(PlannerError::ArchiveAncestryMismatch {
                            ancestor: ancestor.key.clone(),
                            descendant: key.clone(),
                        });
                    }
                    break;
                }
                parent = ancestor.parent_id.clone();
            }
        }
        Ok(Self {
            entries: all
                .values()
                .map(|s| TimelineEntry {
                    key: s.key.clone(),
                    parent_id: s.parent_id.clone(),
                    timestamp_ns: s.timestamp_ns,
                    activity: s.activity(),
                    position: if s.activity() {
                        positions.get(&s.key).copied()
                    } else {
                        None
                    },
                })
                .collect(),
            activity_order: order.into_iter().map(|(_, key)| key).collect(),
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
