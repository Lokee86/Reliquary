//! Historical semantic production over an owner-verified H1 cut.
//! Identity is independent of positions, evidence, policy values and awards.
use crate::MemoryId;
use crate::freshness::historical_planner::{HistoricalTimeline, MemoryAvailability, SourceKey};
use crate::freshness::semantic_policy::{
    SemanticCandidate, SemanticEffect, SemanticPolicy, SemanticPolicyError, SemanticVectorProfile,
    cosine_similarity, plan_local_effects, validate_profile_compatibility,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSemanticSourceVector {
    pub fragment_id: [u8; 32],
    pub components: Vec<f32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSemanticSource {
    pub fragment_id: [u8; 32],
    pub source_keys: BTreeSet<SourceKey>,
    pub evidence_available: bool,
    pub owner_id: String,
    pub profile: SemanticVectorProfile,
    /// Present evidence with no usable rows is completed-empty, not unavailable.
    pub vectors: Vec<HistoricalSemanticSourceVector>,
    pub evidence_digest: [u8; 32],
    /// Derived from complete owner provenance, including generating Episode/cohort.
    pub credit_excluded: BTreeSet<MemoryId>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSemanticTarget {
    pub memory_id: MemoryId,
    pub owner_id: String,
    pub availability: MemoryAvailability,
    pub lifecycle_eligible: bool,
    pub profile: SemanticVectorProfile,
    pub vector: Vec<f32>,
    pub vector_binding_id: [u8; 32],
    pub evidence_digest: [u8; 32],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalSemanticEvent {
    pub event_id: [u8; 32],
    pub fragment_id: [u8; 32],
    pub source_keys: BTreeSet<SourceKey>,
    /// Latest represented activity key; reporting only, not event identity.
    pub source_key: SourceKey,
    pub source_position: u64,
    pub owner_id: String,
    pub semantic_policy_digest: [u8; 32],
    pub compatibility_profile_id: [u8; 32],
    pub source_evidence_digest: [u8; 32],
    pub target_evidence_digest: [u8; 32],
    pub effects: Vec<SemanticEffect>,
    pub credit_excluded: BTreeSet<MemoryId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalSemanticPlan {
    pub events: Vec<HistoricalSemanticEvent>,
    pub coverage: BTreeMap<[u8; 32], String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalSemanticError {
    TimelineSourceMissing(SourceKey),
    DuplicateSource([u8; 32]),
    DuplicateTarget(MemoryId),
    DuplicateFragment([u8; 32]),
    InvalidOwner(String),
    UnsupportedPolicy,
    Policy(SemanticPolicyError),
}
impl std::fmt::Display for HistoricalSemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical semantic: {self:?}")
    }
}
impl std::error::Error for HistoricalSemanticError {}
impl From<SemanticPolicyError> for HistoricalSemanticError {
    fn from(e: SemanticPolicyError) -> Self {
        Self::Policy(e)
    }
}

impl HistoricalSemanticPlan {
    pub fn plan(
        owner_uuid: [u8; 16],
        owner_id: &str,
        policy: SemanticPolicy,
        timeline: &HistoricalTimeline,
        sources: impl IntoIterator<Item = HistoricalSemanticSource>,
        targets: impl IntoIterator<Item = HistoricalSemanticTarget>,
    ) -> Result<Self, HistoricalSemanticError> {
        if owner_id.is_empty() {
            return Err(HistoricalSemanticError::InvalidOwner(owner_id.into()));
        }
        policy.validate()?;
        if policy != SemanticPolicy::default() {
            return Err(HistoricalSemanticError::UnsupportedPolicy);
        }
        let positions: BTreeMap<_, _> = timeline
            .entries
            .iter()
            .map(|e| (e.key.clone(), e.position))
            .collect();
        let mut source_map = BTreeMap::new();
        for source in sources {
            if source.owner_id != owner_id {
                return Err(HistoricalSemanticError::InvalidOwner(source.owner_id));
            }
            for key in &source.source_keys {
                if !positions.contains_key(key) {
                    return Err(HistoricalSemanticError::TimelineSourceMissing(key.clone()));
                }
            }
            if source.vectors.len() > 1
                || source
                    .vectors
                    .iter()
                    .any(|v| v.fragment_id != source.fragment_id)
            {
                return Err(HistoricalSemanticError::DuplicateFragment(
                    source.fragment_id,
                ));
            }
            let id = source.fragment_id;
            if source_map.insert(id, source).is_some() {
                return Err(HistoricalSemanticError::DuplicateSource(id));
            }
        }
        let mut target_map = BTreeMap::new();
        for target in targets {
            if target.owner_id != owner_id {
                return Err(HistoricalSemanticError::InvalidOwner(target.owner_id));
            }
            let id = target.memory_id;
            if target_map.insert(id, target).is_some() {
                return Err(HistoricalSemanticError::DuplicateTarget(id));
            }
        }
        let mut events = Vec::new();
        let mut coverage = BTreeMap::new();
        for (&fragment, source) in &source_map {
            let Some((position, key)) = source
                .source_keys
                .iter()
                .filter_map(|key| positions.get(key).copied().flatten().map(|at| (at, key)))
                .max_by_key(|(at, _)| *at)
            else {
                coverage.insert(fragment, "non-activity-only-fragment".into());
                continue;
            };
            if !source.evidence_available {
                coverage.insert(fragment, "fragment-evidence-unavailable".into());
                continue;
            }
            let usable: Vec<_> = source
                .vectors
                .iter()
                .filter(|v| {
                    v.components.len() == source.profile.dimensions as usize
                        && cosine_similarity(&v.components, &v.components).is_ok()
                })
                .collect();
            if usable.is_empty() {
                coverage.insert(fragment, "complete-no-usable-source-rows".into());
                continue;
            }
            let mut candidates = Vec::new();
            let (mut considered, mut future, mut ineligible, mut incompatible) =
                (0u64, 0u64, 0u64, 0u64);
            let mut digest = Sha256::new();
            digest.update(b"reliquary:historical-semantic-targets:v1\0");
            for target in target_map.values() {
                if !target.lifecycle_eligible || source.credit_excluded.contains(&target.memory_id)
                {
                    ineligible += 1;
                    continue;
                }
                let target_position = match target.availability {
                    MemoryAvailability::Supported { position } if position > 0 => position,
                    _ => {
                        ineligible += 1;
                        continue;
                    }
                };
                if target_position > position {
                    future += 1;
                    continue;
                }
                considered += 1;
                digest.update(target.memory_id.0);
                digest.update(target.evidence_digest);
                if validate_profile_compatibility(source.profile, target.profile).is_err()
                    || target.vector.len() != source.profile.dimensions as usize
                {
                    incompatible += 1;
                    continue;
                }
                for vector in &usable {
                    let Ok(cosine) = cosine_similarity(&vector.components, &target.vector) else {
                        continue;
                    };
                    candidates.push(SemanticCandidate {
                        memory_id: target.memory_id,
                        cosine,
                        fragment_id: vector.fragment_id.to_vec(),
                        vector_binding_id: target.vector_binding_id,
                        profile_compatible: true,
                        generation_valid: true,
                        source_excluded: false,
                    });
                }
            }
            let effects = plan_local_effects(policy, candidates)?;
            if effects.is_empty() {
                coverage.insert(fragment,format!("complete-no-eligible-match:considered={considered};future={future};ineligible={ineligible};profile={incompatible}"));
                continue;
            }
            events.push(HistoricalSemanticEvent {
                event_id: historical_semantic_event_id(owner_uuid, fragment),
                fragment_id: fragment,
                source_keys: source.source_keys.clone(),
                source_key: key.clone(),
                source_position: position,
                owner_id: owner_id.into(),
                semantic_policy_digest: policy.digest()?,
                compatibility_profile_id: source.profile.profile_id,
                source_evidence_digest: source.evidence_digest,
                target_evidence_digest: digest.finalize().into(),
                effects,
                credit_excluded: source.credit_excluded.clone(),
            });
            coverage.insert(fragment, "event-planned".into());
        }
        events.sort_by_key(|e| (e.source_position, e.event_id));
        Ok(Self { events, coverage })
    }
}
/// Fixed import-semantic fragment domain; generation/profile/import are replay evidence.
pub fn historical_semantic_event_id(owner: [u8; 16], fragment: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-semantic-fragment:v1\0");
    hash.update(owner);
    hash.update(fragment);
    hash.finalize().into()
}
#[cfg(test)]
#[path = "historical_semantic_tests.rs"]
mod tests;
