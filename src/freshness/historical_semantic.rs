//! Pure import-only historical semantic event planning.
//!
//! This is intentionally separate from Freshness storage, propagation, and
//! scoring. It consumes the Step 14 provenance plan and Step 15 clock proof,
//! reuses the live F8 numerical policy, and emits deterministic event plans.

use crate::MemoryId;
use crate::freshness::historical_planner::{
    HistoricalProvenancePlan, HistoricalTimeline, MemoryAvailability, SourceKey,
};
use crate::freshness::semantic_policy::{
    SemanticCandidate, SemanticEffect, SemanticPolicy, SemanticPolicyError, SemanticVectorProfile,
    plan_local_effects, validate_profile_compatibility,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSemanticSource {
    pub key: SourceKey,
    pub owner_id: String,
    pub profile: SemanticVectorProfile,
    pub vector: Vec<f32>,
    pub evidence_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSemanticTarget {
    pub memory_id: MemoryId,
    pub owner_id: String,
    pub availability: MemoryAvailability,
    pub lifecycle_eligible: bool,
    pub source_key: Option<SourceKey>,
    pub profile: SemanticVectorProfile,
    pub vector: Vec<f32>,
    pub fragment_id: Vec<u8>,
    pub vector_binding_id: [u8; 32],
    pub evidence_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalSemanticEvent {
    pub event_id: String,
    pub source_key: SourceKey,
    pub source_position: u64,
    pub owner_id: String,
    pub semantic_policy_digest: [u8; 32],
    pub compatibility_profile_id: [u8; 32],
    pub source_evidence_digest: [u8; 32],
    pub target_evidence_digest: [u8; 32],
    pub effects: Vec<SemanticEffect>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalSemanticPlan {
    pub events: Vec<HistoricalSemanticEvent>,
    pub coverage: BTreeMap<SourceKey, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalSemanticError {
    TimelineSourceMissing(SourceKey),
    DuplicateSource(SourceKey),
    DuplicateTarget(MemoryId),
    InvalidOwner(String),
    InvalidSourcePosition(SourceKey),
    Policy(SemanticPolicyError),
}

impl std::fmt::Display for HistoricalSemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimelineSourceMissing(key) => write!(
                f,
                "source is not in the reconstructed timeline: {}/{}",
                key.conversation_id, key.node_id
            ),
            Self::DuplicateSource(key) => write!(
                f,
                "duplicate semantic source {}/{}",
                key.conversation_id, key.node_id
            ),
            Self::DuplicateTarget(id) => write!(f, "duplicate semantic target {id:?}"),
            Self::InvalidOwner(owner) => write!(f, "invalid historical owner {owner}"),
            Self::InvalidSourcePosition(key) => write!(
                f,
                "source has no accepted activity position: {}",
                key.node_id
            ),
            Self::Policy(error) => write!(f, "semantic policy error: {error}"),
        }
    }
}
impl std::error::Error for HistoricalSemanticError {}
impl From<SemanticPolicyError> for HistoricalSemanticError {
    fn from(error: SemanticPolicyError) -> Self {
        Self::Policy(error)
    }
}

impl HistoricalSemanticPlan {
    pub fn plan(
        owner_id: &str,
        policy: SemanticPolicy,
        timeline: &HistoricalTimeline,
        provenance: &HistoricalProvenancePlan,
        sources: impl IntoIterator<Item = HistoricalSemanticSource>,
        targets: impl IntoIterator<Item = HistoricalSemanticTarget>,
    ) -> Result<Self, HistoricalSemanticError> {
        if owner_id.trim().is_empty() {
            return Err(HistoricalSemanticError::InvalidOwner(owner_id.into()));
        }
        policy.validate()?;
        let mut source_map = BTreeMap::new();
        for source in sources {
            if source.owner_id != owner_id {
                return Err(HistoricalSemanticError::InvalidOwner(source.owner_id));
            }
            let key = source.key.clone();
            if source_map.insert(key.clone(), source).is_some() {
                return Err(HistoricalSemanticError::DuplicateSource(key));
            }
        }
        let mut target_map = BTreeMap::new();
        for target in targets {
            if target.owner_id != owner_id {
                return Err(HistoricalSemanticError::InvalidOwner(target.owner_id));
            }
            let memory_id = target.memory_id;
            if target_map.insert(memory_id, target).is_some() {
                return Err(HistoricalSemanticError::DuplicateTarget(memory_id));
            }
        }
        // The target set is the immutable, already-resolved projection of the provenance plan.
        // Its availability and lifecycle fields are checked below; the planner does not
        // reinterpret Memory identifiers or invent missing evidence.
        let _ = provenance;
        let mut events = Vec::new();
        let mut coverage = BTreeMap::new();
        for source_key in timeline.activity_order.iter() {
            let Some(source) = source_map.get(source_key) else {
                coverage.insert(source_key.clone(), "source-evidence-unavailable".into());
                continue;
            };
            let Some(position) = timeline.position(source_key) else {
                return Err(HistoricalSemanticError::InvalidSourcePosition(
                    source_key.clone(),
                ));
            };
            let Some(source_vector_len) =
                (!source.vector.is_empty()).then_some(source.vector.len())
            else {
                coverage.insert(source_key.clone(), "source-vector-unavailable".into());
                continue;
            };
            let mut candidates = Vec::new();
            let mut considered = 0_u64;
            let mut skipped_future = 0_u64;
            let mut skipped_ineligible = 0_u64;
            let mut skipped_profile = 0_u64;
            let mut target_digest = Sha256::new();
            for target in target_map.values() {
                if !target.lifecycle_eligible {
                    skipped_ineligible += 1;
                    continue;
                }
                let target_position = match target.availability {
                    MemoryAvailability::Supported { position } => position,
                    MemoryAvailability::Unsupported | MemoryAvailability::CreditExcluded => {
                        skipped_ineligible += 1;
                        continue;
                    }
                };
                if target_position > position {
                    skipped_future += 1;
                    continue;
                }
                if target.source_key.as_ref() == Some(source_key) {
                    skipped_ineligible += 1;
                    continue;
                }
                considered += 1;
                target_digest.update(target.memory_id.0);
                target_digest.update(&target.evidence_digest);
                let compatible = validate_profile_compatibility(source.profile, target.profile)
                    .is_ok()
                    && source_vector_len == target.vector.len();
                if !compatible {
                    skipped_profile += 1;
                }
                candidates.push(SemanticCandidate {
                    memory_id: target.memory_id,
                    cosine: cosine_or_zero(&source.vector, &target.vector),
                    fragment_id: target.fragment_id.clone(),
                    vector_binding_id: target.vector_binding_id,
                    profile_compatible: compatible,
                    generation_valid: target.profile.generation != 0,
                    source_excluded: false,
                });
            }
            let effects = plan_local_effects(policy, candidates)?;
            if effects.is_empty() {
                coverage.insert(source_key.clone(), format!("no-eligible-match:considered={considered};future={skipped_future};ineligible={skipped_ineligible};profile={skipped_profile}"));
                continue;
            }
            let target_evidence_digest: [u8; 32] = target_digest.finalize().into();
            let event_id = event_id(owner_id, source_key, position, policy, &effects);
            events.push(HistoricalSemanticEvent {
                event_id,
                source_key: source_key.clone(),
                source_position: position,
                owner_id: owner_id.into(),
                semantic_policy_digest: policy.digest().expect("validated policy"),
                compatibility_profile_id: source.profile.profile_id,
                source_evidence_digest: source.evidence_digest,
                target_evidence_digest,
                effects,
            });
            coverage.insert(source_key.clone(), "event-planned".into());
        }
        events.sort_by(|left, right| {
            left.source_position
                .cmp(&right.source_position)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        Ok(Self { events, coverage })
    }
}

fn cosine_or_zero(source: &[f32], target: &[f32]) -> f64 {
    crate::freshness::semantic_policy::cosine_similarity(source, target).unwrap_or(-2.0)
}

fn event_id(
    owner: &str,
    source: &SourceKey,
    position: u64,
    policy: SemanticPolicy,
    effects: &[SemanticEffect],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"reliquary/historical-semantic-event/v1\0");
    put(&mut hasher, owner.as_bytes());
    put(&mut hasher, source.conversation_id.as_bytes());
    put(&mut hasher, source.node_id.as_bytes());
    hasher.update(position.to_le_bytes());
    hasher.update(policy.digest().expect("validated policy"));
    for effect in effects {
        hasher.update(effect.memory_id.0);
        hasher.update(effect.cosine_micros.to_le_bytes());
        hasher.update(effect.bonus.to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn put(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freshness::historical_planner::{
        HistoricalProvenancePlan, HistoricalSource, MemoryPlan,
    };

    fn key(conversation_id: &str, node_id: &str) -> SourceKey {
        SourceKey {
            conversation_id: conversation_id.into(),
            node_id: node_id.into(),
        }
    }
    fn id(byte: u8) -> MemoryId {
        MemoryId([byte; 32])
    }
    fn profile() -> SemanticVectorProfile {
        SemanticVectorProfile {
            profile_id: [9; 32],
            dimensions: 2,
            generation: 1,
        }
    }
    fn timeline() -> HistoricalTimeline {
        HistoricalTimeline::plan(vec![
            HistoricalSource {
                key: key("c", "u"),
                parent_id: None,
                role: "user".into(),
                timestamp_ns: 1,
            },
            HistoricalSource {
                key: key("c", "a"),
                parent_id: Some("u".into()),
                role: "assistant".into(),
                timestamp_ns: 2,
            },
        ])
        .unwrap()
    }
    fn provenance() -> HistoricalProvenancePlan {
        HistoricalProvenancePlan {
            memories: vec![
                MemoryPlan {
                    memory_id: format!("{:?}", id(1)),
                    availability: MemoryAvailability::Supported { position: 1 },
                    cohort_id: "m1".into(),
                },
                MemoryPlan {
                    memory_id: format!("{:?}", id(2)),
                    availability: MemoryAvailability::Supported { position: 1 },
                    cohort_id: "m2".into(),
                },
            ],
            coverage: BTreeMap::new(),
        }
    }
    fn target(byte: u8, vector: Vec<f32>, source: Option<SourceKey>) -> HistoricalSemanticTarget {
        HistoricalSemanticTarget {
            memory_id: id(byte),
            owner_id: "owner".into(),
            availability: MemoryAvailability::Supported { position: 1 },
            lifecycle_eligible: true,
            source_key: source,
            profile: profile(),
            vector,
            fragment_id: vec![byte],
            vector_binding_id: [byte; 32],
            evidence_digest: [byte; 32],
        }
    }
    #[test]
    fn plans_only_eligible_historical_matches() {
        let timeline = timeline();
        let mut p = provenance();
        p.memories[0].memory_id = format!("{:?}", id(1));
        let result = HistoricalSemanticPlan::plan(
            "owner",
            SemanticPolicy::default(),
            &timeline,
            &p,
            [HistoricalSemanticSource {
                key: key("c", "u"),
                owner_id: "owner".into(),
                profile: profile(),
                vector: vec![1.0, 0.0],
                evidence_digest: [1; 32],
            }],
            [target(2, vec![1.0, 0.0], None)],
        )
        .unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].effects[0].bonus, 10);
    }
    #[test]
    fn future_and_ineligible_targets_do_not_consume_candidate_budget() {
        let timeline = timeline();
        let p = provenance();
        let mut future = target(1, vec![1.0, 0.0], None);
        future.availability = MemoryAvailability::Supported { position: 2 };
        let mut invalid = target(2, vec![1.0, 0.0], None);
        invalid.lifecycle_eligible = false;
        let result = HistoricalSemanticPlan::plan(
            "owner",
            SemanticPolicy {
                candidate_limit: 1,
                ..SemanticPolicy::default()
            },
            &timeline,
            &p,
            [HistoricalSemanticSource {
                key: key("c", "u"),
                owner_id: "owner".into(),
                profile: profile(),
                vector: vec![1.0, 0.0],
                evidence_digest: [1; 32],
            }],
            [future, invalid, target(3, vec![1.0, 0.0], None)],
        )
        .unwrap();
        assert_eq!(result.events[0].effects.len(), 1);
        assert_eq!(result.events[0].effects[0].memory_id, id(3));
    }
    #[test]
    fn source_without_vector_is_covered_without_event() {
        let timeline = timeline();
        let p = provenance();
        let result = HistoricalSemanticPlan::plan(
            "owner",
            SemanticPolicy::default(),
            &timeline,
            &p,
            [HistoricalSemanticSource {
                key: key("c", "u"),
                owner_id: "owner".into(),
                profile: profile(),
                vector: vec![],
                evidence_digest: [1; 32],
            }],
            std::iter::empty(),
        )
        .unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.coverage[&key("c", "u")], "source-vector-unavailable");
    }
    #[test]
    fn incompatible_profile_is_visible_and_does_not_win() {
        let timeline = timeline();
        let p = provenance();
        let mut target = target(1, vec![1.0, 0.0], None);
        target.profile.profile_id = [8; 32];
        let result = HistoricalSemanticPlan::plan(
            "owner",
            SemanticPolicy::default(),
            &timeline,
            &p,
            [HistoricalSemanticSource {
                key: key("c", "u"),
                owner_id: "owner".into(),
                profile: profile(),
                vector: vec![1.0, 0.0],
                evidence_digest: [1; 32],
            }],
            [target],
        )
        .unwrap();
        assert!(result.events.is_empty());
        assert!(result.coverage[&key("c", "u")].contains("profile=1"));
    }
}
