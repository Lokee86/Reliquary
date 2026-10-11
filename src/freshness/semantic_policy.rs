//! Graph-independent semantic Freshness policy primitives.
//!
//! This module deliberately owns only deterministic numerical policy and
//! candidate reduction. Archive ingestion, vector storage, lifecycle rules,
//! and graph propagation remain in their existing owners.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::MemoryId;

pub const SEMANTIC_POLICY_VERSION: u16 = 1;
pub const SEMANTIC_NUMERICAL_REVISION: u16 = 1;
pub const SEMANTIC_ELIGIBILITY_REVISION: u16 = 1;
pub const SEMANTIC_THRESHOLD: f64 = 0.75;
pub const SEMANTIC_BONUS_CAP: i16 = 10;
pub const SEMANTIC_CANDIDATE_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCurve {
    InclusiveLinearV1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SemanticPolicy {
    pub version: u16,
    pub numerical_revision: u16,
    pub eligibility_revision: u16,
    pub threshold: f64,
    pub bonus_cap: i16,
    pub candidate_limit: usize,
    pub curve: SemanticCurve,
}

impl Default for SemanticPolicy {
    fn default() -> Self {
        Self {
            version: SEMANTIC_POLICY_VERSION,
            numerical_revision: SEMANTIC_NUMERICAL_REVISION,
            eligibility_revision: SEMANTIC_ELIGIBILITY_REVISION,
            threshold: SEMANTIC_THRESHOLD,
            bonus_cap: SEMANTIC_BONUS_CAP,
            candidate_limit: SEMANTIC_CANDIDATE_LIMIT,
            curve: SemanticCurve::InclusiveLinearV1,
        }
    }
}

impl SemanticPolicy {
    pub fn validate(self) -> Result<(), SemanticPolicyError> {
        if self.version != SEMANTIC_POLICY_VERSION
            || self.numerical_revision != SEMANTIC_NUMERICAL_REVISION
            || self.eligibility_revision != SEMANTIC_ELIGIBILITY_REVISION
        {
            return Err(SemanticPolicyError::UnsupportedRevision);
        }
        if !self.threshold.is_finite() || !(0.0..1.0).contains(&self.threshold) {
            return Err(SemanticPolicyError::InvalidThreshold);
        }
        if !(1..=100).contains(&self.bonus_cap) {
            return Err(SemanticPolicyError::InvalidBonusCap);
        }
        if self.candidate_limit == 0 {
            return Err(SemanticPolicyError::InvalidCandidateLimit);
        }
        Ok(())
    }

    pub fn canonical_descriptor(self) -> Result<String, SemanticPolicyError> {
        self.validate()?;
        Ok(format!(
            r#"{{"bonus_cap":{},"candidate_limit":{},"curve":"inclusive-linear-v1","eligibility_revision":{},"numerical_revision":{},"threshold":{},"version":{}}}"#,
            self.bonus_cap,
            self.candidate_limit,
            self.eligibility_revision,
            self.numerical_revision,
            format_threshold(self.threshold),
            self.version
        ))
    }

    pub fn digest(self) -> Result<[u8; 32], SemanticPolicyError> {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_descriptor()?.as_bytes());
        Ok(hasher.finalize().into())
    }

    pub fn bonus(self, cosine: f64) -> Result<i16, SemanticPolicyError> {
        self.validate()?;
        validate_cosine(cosine)?;
        if cosine < self.threshold {
            return Ok(0);
        }
        let span = 1.0 - self.threshold;
        let raw = 1.0 + ((cosine - self.threshold) * f64::from(self.bonus_cap - 1) / span).floor();
        Ok(raw.min(f64::from(self.bonus_cap)) as i16)
    }
}

fn format_threshold(value: f64) -> String {
    let mut rendered = format!("{value:.12}");
    while rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.push('0');
    }
    rendered
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticPolicyError {
    UnsupportedRevision,
    InvalidThreshold,
    InvalidBonusCap,
    InvalidCandidateLimit,
    InvalidCosine,
    EmptyVector,
    DimensionMismatch,
    NonFiniteVector,
    ZeroNormVector,
    NonFiniteSimilarity,
    IncompatibleProfile,
    InvalidGeneration,
}

impl fmt::Display for SemanticPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SemanticPolicyError {}

pub fn validate_cosine(cosine: f64) -> Result<(), SemanticPolicyError> {
    if !cosine.is_finite() || !(-1.0..=1.0).contains(&cosine) {
        return Err(SemanticPolicyError::InvalidCosine);
    }
    Ok(())
}

pub fn cosine_similarity(source: &[f32], target: &[f32]) -> Result<f64, SemanticPolicyError> {
    if source.is_empty() || target.is_empty() {
        return Err(SemanticPolicyError::EmptyVector);
    }
    if source.len() != target.len() {
        return Err(SemanticPolicyError::DimensionMismatch);
    }

    let mut dot = 0.0_f64;
    let mut source_norm = 0.0_f64;
    let mut target_norm = 0.0_f64;
    for (&left, &right) in source.iter().zip(target) {
        if !left.is_finite() || !right.is_finite() {
            return Err(SemanticPolicyError::NonFiniteVector);
        }
        let left = f64::from(left);
        let right = f64::from(right);
        dot += left * right;
        source_norm += left * left;
        target_norm += right * right;
    }
    if source_norm == 0.0 || target_norm == 0.0 {
        return Err(SemanticPolicyError::ZeroNormVector);
    }
    let similarity = dot / (source_norm.sqrt() * target_norm.sqrt());
    validate_cosine(similarity).map_err(|_| SemanticPolicyError::NonFiniteSimilarity)?;
    Ok(similarity)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticVectorProfile {
    pub profile_id: [u8; 32],
    pub dimensions: u32,
    pub generation: u64,
}

pub fn validate_profile_compatibility(
    source: SemanticVectorProfile,
    target: SemanticVectorProfile,
) -> Result<(), SemanticPolicyError> {
    if source.generation == 0 || target.generation == 0 {
        return Err(SemanticPolicyError::InvalidGeneration);
    }
    if source.profile_id != target.profile_id || source.dimensions != target.dimensions {
        return Err(SemanticPolicyError::IncompatibleProfile);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticCandidate {
    pub memory_id: MemoryId,
    pub cosine: f64,
    pub fragment_id: Vec<u8>,
    pub vector_binding_id: [u8; 32],
    pub profile_compatible: bool,
    pub generation_valid: bool,
    pub source_excluded: bool,
}

impl SemanticCandidate {
    fn usable(&self) -> bool {
        self.profile_compatible
            && self.generation_valid
            && !self.source_excluded
            && validate_cosine(self.cosine).is_ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticEffect {
    pub memory_id: MemoryId,
    pub cosine_micros: i64,
    pub bonus: i16,
}

pub fn plan_local_effects(
    policy: SemanticPolicy,
    candidates: impl IntoIterator<Item = SemanticCandidate>,
) -> Result<Vec<SemanticEffect>, SemanticPolicyError> {
    policy.validate()?;
    let mut strongest: BTreeMap<MemoryId, SemanticCandidate> = BTreeMap::new();
    for candidate in candidates {
        if !candidate.usable() {
            continue;
        }
        let replace = strongest
            .get(&candidate.memory_id)
            .map_or(true, |current| stronger_match(&candidate, current));
        if replace {
            strongest.insert(candidate.memory_id, candidate);
        }
    }

    let mut ranked = strongest
        .into_values()
        .filter_map(|candidate| {
            let bonus = policy.bonus(candidate.cosine).ok()?;
            (bonus > 0).then_some((candidate, bonus))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left, _), (right, _)| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| {
                right
                    .0
                    .cosine
                    .partial_cmp(&left.0.cosine)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.0.memory_id.cmp(&right.0.memory_id))
    });
    ranked.truncate(policy.candidate_limit);

    let mut effects = ranked
        .into_iter()
        .map(|(candidate, bonus)| SemanticEffect {
            memory_id: candidate.memory_id,
            cosine_micros: (candidate.cosine * 1_000_000.0).round() as i64,
            bonus,
        })
        .collect::<Vec<_>>();
    effects.sort_by_key(|effect| effect.memory_id);
    Ok(effects)
}

fn stronger_match(candidate: &SemanticCandidate, current: &SemanticCandidate) -> bool {
    match candidate.cosine.partial_cmp(&current.cosine) {
        Some(Ordering::Greater) => true,
        Some(Ordering::Less) => false,
        _ => {
            candidate
                .fragment_id
                .cmp(&current.fragment_id)
                .then_with(|| candidate.vector_binding_id.cmp(&current.vector_binding_id))
                == Ordering::Less
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> MemoryId {
        MemoryId([byte; 32])
    }

    fn candidate(memory: u8, cosine: f64) -> SemanticCandidate {
        SemanticCandidate {
            memory_id: id(memory),
            cosine,
            fragment_id: vec![memory],
            vector_binding_id: [memory; 32],
            profile_compatible: true,
            generation_valid: true,
            source_excluded: false,
        }
    }

    #[test]
    fn threshold_is_inclusive_and_curve_is_monotone() {
        let policy = SemanticPolicy::default();
        assert_eq!(policy.bonus(0.749_999).unwrap(), 0);
        assert_eq!(policy.bonus(0.75).unwrap(), 1);
        let mut previous = 0;
        for step in 0..=10_000 {
            let cosine = 0.75 + (step as f64 / 10_000.0) * 0.25;
            let bonus = policy.bonus(cosine).unwrap();
            assert!(bonus >= previous);
            previous = bonus;
        }
        assert_eq!(policy.bonus(1.0).unwrap(), 10);
    }

    #[test]
    fn bonus_is_capped() {
        assert_eq!(
            SemanticPolicy::default().bonus(1.0).unwrap(),
            SEMANTIC_BONUS_CAP
        );
    }

    #[test]
    fn strongest_match_wins_and_ties_use_bytes() {
        let mut weaker = candidate(1, 0.8);
        weaker.fragment_id = vec![9];
        let mut stronger = candidate(1, 0.9);
        stronger.fragment_id = vec![8];
        let effects = plan_local_effects(SemanticPolicy::default(), [weaker, stronger]).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].bonus, 10);
    }

    #[test]
    fn equal_similarity_tie_uses_fragment_then_binding() {
        let mut first = candidate(1, 0.8);
        first.fragment_id = vec![2];
        let mut second = candidate(1, 0.8);
        second.fragment_id = vec![1];
        let effects = plan_local_effects(SemanticPolicy::default(), [first, second]).unwrap();
        assert_eq!(effects[0].cosine_micros, 800_000);
    }

    #[test]
    fn incompatible_and_excluded_candidates_do_not_consume_budget() {
        let mut bad_profile = candidate(1, 0.99);
        bad_profile.profile_compatible = false;
        let mut bad_generation = candidate(2, 0.99);
        bad_generation.generation_valid = false;
        let mut excluded = candidate(3, 0.99);
        excluded.source_excluded = true;
        let effects = plan_local_effects(
            SemanticPolicy {
                candidate_limit: 1,
                ..SemanticPolicy::default()
            },
            [bad_profile, bad_generation, excluded, candidate(4, 0.75)],
        )
        .unwrap();
        assert_eq!(
            effects
                .iter()
                .map(|effect| effect.memory_id)
                .collect::<Vec<_>>(),
            vec![id(4)]
        );
    }

    #[test]
    fn candidate_limit_ranks_before_id_order() {
        let effects = plan_local_effects(
            SemanticPolicy {
                candidate_limit: 2,
                ..SemanticPolicy::default()
            },
            [candidate(3, 0.76), candidate(1, 0.99), candidate(2, 0.8)],
        )
        .unwrap();
        assert_eq!(
            effects
                .iter()
                .map(|effect| effect.memory_id)
                .collect::<Vec<_>>(),
            vec![id(1), id(2)]
        );
    }

    #[test]
    fn invalid_policy_and_cosines_are_rejected_or_dropped() {
        assert_eq!(
            SemanticPolicy {
                threshold: f64::NAN,
                ..SemanticPolicy::default()
            }
            .bonus(0.8),
            Err(SemanticPolicyError::InvalidThreshold)
        );
        assert_eq!(
            validate_cosine(f64::NAN),
            Err(SemanticPolicyError::InvalidCosine)
        );
        let effects =
            plan_local_effects(SemanticPolicy::default(), [candidate(1, f64::NAN)]).unwrap();
        assert!(effects.is_empty());
    }

    #[test]
    fn profile_and_generation_checks_are_explicit() {
        let base = SemanticVectorProfile {
            profile_id: [1; 32],
            dimensions: 3,
            generation: 1,
        };
        assert!(validate_profile_compatibility(base, base).is_ok());
        assert_eq!(
            validate_profile_compatibility(
                base,
                SemanticVectorProfile {
                    profile_id: [2; 32],
                    ..base
                }
            ),
            Err(SemanticPolicyError::IncompatibleProfile)
        );
        assert_eq!(
            validate_profile_compatibility(
                base,
                SemanticVectorProfile {
                    generation: 0,
                    ..base
                }
            ),
            Err(SemanticPolicyError::InvalidGeneration)
        );
    }

    #[test]
    fn cosine_validation_is_deterministic() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]).unwrap(), 1.0);
        assert_eq!(
            cosine_similarity(&[1.0], &[0.0]),
            Err(SemanticPolicyError::ZeroNormVector)
        );
        assert_eq!(
            cosine_similarity(&[1.0], &[1.0, 2.0]),
            Err(SemanticPolicyError::DimensionMismatch)
        );
        assert_eq!(
            cosine_similarity(&[], &[]),
            Err(SemanticPolicyError::EmptyVector)
        );
    }

    #[test]
    fn canonical_descriptor_has_frozen_digest() {
        let policy = SemanticPolicy::default();
        assert_eq!(
            policy.canonical_descriptor().unwrap(),
            r#"{"bonus_cap":10,"candidate_limit":64,"curve":"inclusive-linear-v1","eligibility_revision":1,"numerical_revision":1,"threshold":0.75,"version":1}"#
        );
        assert_eq!(
            policy.digest().unwrap(),
            [
                0x58, 0x86, 0xdc, 0x23, 0xe3, 0x86, 0x66, 0x4b, 0x3d, 0xbe, 0xf0, 0x3a, 0x11, 0x6d,
                0xbf, 0xab, 0x55, 0x67, 0xad, 0x29, 0x98, 0xbe, 0x0e, 0x21, 0xe0, 0x7c, 0x48, 0x95,
                0xab, 0x1d, 0x26, 0x1f,
            ]
        );
    }
}
