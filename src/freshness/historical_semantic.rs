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
    pub key: SourceKey,
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
    pub coverage: BTreeMap<SourceKey, String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalSemanticError {
    TimelineSourceMissing(SourceKey),
    DuplicateSource(SourceKey),
    DuplicateTarget(MemoryId),
    DuplicateFragment(SourceKey),
    InvalidOwner(String),
    InvalidSourcePosition(SourceKey),
    IdentityOverflow,
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
        import_id: [u8; 32],
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
        let mut source_map = BTreeMap::new();
        for mut source in sources {
            if source.owner_id != owner_id {
                return Err(HistoricalSemanticError::InvalidOwner(source.owner_id));
            }
            if timeline.position(&source.key).is_none() {
                return Err(HistoricalSemanticError::TimelineSourceMissing(source.key));
            }
            source
                .vectors
                .sort_by(|a, b| a.fragment_id.cmp(&b.fragment_id));
            if source
                .vectors
                .windows(2)
                .any(|v| v[0].fragment_id == v[1].fragment_id)
            {
                return Err(HistoricalSemanticError::DuplicateFragment(source.key));
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
            let id = target.memory_id;
            if target_map.insert(id, target).is_some() {
                return Err(HistoricalSemanticError::DuplicateTarget(id));
            }
        }
        let mut events = Vec::new();
        let mut coverage = BTreeMap::new();
        for key in &timeline.activity_order {
            let Some(source) = source_map.get(key) else {
                coverage.insert(key.clone(), "source-evidence-unavailable".into());
                continue;
            };
            let position = timeline
                .position(key)
                .ok_or_else(|| HistoricalSemanticError::InvalidSourcePosition(key.clone()))?;
            let usable: Vec<_> = source
                .vectors
                .iter()
                .filter(|v| {
                    v.components.len() == source.profile.dimensions as usize
                        && cosine_similarity(&v.components, &v.components).is_ok()
                })
                .collect();
            if usable.is_empty() {
                coverage.insert(key.clone(), "complete-no-usable-source-rows".into());
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
                coverage.insert(key.clone(),format!("complete-no-eligible-match:considered={considered};future={future};ineligible={ineligible};profile={incompatible}"));
                continue;
            }
            events.push(HistoricalSemanticEvent {
                event_id: historical_semantic_event_id(owner_uuid, import_id, key)?,
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
            coverage.insert(key.clone(), "event-planned".into());
        }
        events.sort_by_key(|e| (e.source_position, e.event_id));
        Ok(Self { events, coverage })
    }
}
/// Exact H1 identity map. Positions, roots, amounts and manifest hash are absent.
pub fn historical_semantic_event_id(
    owner: [u8; 16],
    import: [u8; 32],
    source: &SourceKey,
) -> Result<[u8; 32], HistoricalSemanticError> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-semantic:v1\0");
    hash.update([7]);
    hash.update(4u32.to_le_bytes());
    for (key, tag, bytes) in [
        ("conversation_id", 5, source.conversation_id.as_bytes()),
        ("import_id", 4, import.as_slice()),
        ("node_id", 5, source.node_id.as_bytes()),
        ("owner_uuid", 4, owner.as_slice()),
    ] {
        hash.update([5]);
        hash.update((key.len() as u32).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update([tag]);
        hash.update(
            u32::try_from(bytes.len())
                .map_err(|_| HistoricalSemanticError::IdentityOverflow)?
                .to_le_bytes(),
        );
        hash.update(bytes);
    }
    Ok(hash.finalize().into())
}
#[cfg(test)]
#[path = "historical_semantic_tests.rs"]
mod tests;
