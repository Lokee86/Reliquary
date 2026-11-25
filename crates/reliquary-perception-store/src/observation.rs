//! Canonical owner-local Observation records and their derived indexes.

mod projection;
pub use projection::{ObservationRecord, ObservationStore};

use super::{EntityId, MemoryEntityMentionKey, MemoryTextField, PerceptionError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_OBSERVATION_OPERATIONS: usize = 32;
pub const MAX_OBSERVATION_CLAIM_VECTOR_ROWS: usize = 16;
pub const MAX_OBSERVATION_LINEAGE: usize = 64;
pub const MAX_OBSERVATION_DEPENDENCIES: usize = 4096;
const MAX_CITATIONS: usize = 8;
const MAX_REVIEWED: usize = 11;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ObservationId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObservationSource {
    Memory([u8; 32]),
    Observation(ObservationId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ObservationCitation {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub source: ObservationSource,
    pub revision: u64,
    pub body_id: [u8; 32],
    pub field: MemoryTextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ObservationMemoryLineage {
    pub memory_id: [u8; 32],
    pub revision: u64,
    pub body_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObservationStatus {
    Observed,
    Planned,
    Requested,
    Possible,
    Expected,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObservationTimeScope {
    Past,
    Current,
    Future,
    Unspecified,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ObservationValidity {
    pub start_ns: Option<i64>,
    pub end_ns: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationMention {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub retained_ordinal: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationParticipant {
    pub entity_id: EntityId,
    pub revision: u64,
    pub role: Option<String>,
    pub mention: ObservationMention,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObservationProfile {
    Ordinary,
    Relational(Vec<ObservationParticipant>),
}

impl ObservationProfile {
    pub fn participants(&self) -> &[ObservationParticipant] {
        match self {
            Self::Ordinary => &[],
            Self::Relational(participants) => participants,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObservationReceptorKind {
    Support,
    Challenge,
    Qualification,
    Change,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationReceptor {
    pub kind: ObservationReceptorKind,
    pub description: String,
    pub embedding_profile: [u8; 32],
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationDraft {
    pub proposition: String,
    pub profile: ObservationProfile,
    pub status: ObservationStatus,
    pub time_scope: ObservationTimeScope,
    pub validity: Option<ObservationValidity>,
    pub citations: Vec<ObservationCitation>,
    pub reviewed: Vec<ObservationCitation>,
    pub receptors: Vec<ObservationReceptor>,
}

impl ObservationDraft {
    pub fn body_id(&self) -> [u8; 32] {
        digest(
            b"reliquary-observation-body-v1",
            &[self.proposition.as_bytes()],
        )
    }

    pub fn same_verified_claim(&self, other: &Self) -> bool {
        self.proposition == other.proposition
            && self.profile == other.profile
            && self.status == other.status
            && self.time_scope == other.time_scope
            && self.validity == other.validity
            && self.citations.iter().collect::<BTreeSet<_>>()
                == other.citations.iter().collect::<BTreeSet<_>>()
    }

    pub fn validate(&self) -> Result<(), PerceptionError> {
        if !bounded_text(&self.proposition, 4096)
            || self.citations.is_empty()
            || self.citations.len() > MAX_CITATIONS
            || self.citations.iter().collect::<BTreeSet<_>>().len() != self.citations.len()
            || !valid_reviewed(&self.reviewed)
            || self
                .validity
                .is_some_and(|scope| match (scope.start_ns, scope.end_ns) {
                    (Some(start), Some(end)) => start >= end,
                    (None, None) => true,
                    _ => false,
                })
        {
            return Err(invalid("Observation draft"));
        }
        if let ObservationProfile::Relational(participants) = &self.profile
            && (!(2..=4).contains(&participants.len())
                || participants
                    .iter()
                    .map(|p| p.entity_id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != participants.len()
                || participants.iter().any(|p| {
                    p.revision == 0
                        || p.mention.revision == 0
                        || p.mention.key.start_byte >= p.mention.key.end_byte
                        || p.role.as_ref().is_some_and(|role| !bounded_text(role, 128))
                }))
        {
            return Err(invalid("Observation participants"));
        }
        let Some(primary) = self.receptors.first() else {
            return Err(invalid("Observation receptors"));
        };
        let mut receptors = std::collections::BTreeMap::new();
        for receptor in &self.receptors {
            let count = receptors.entry(receptor.kind).or_insert(0);
            *count += 1;
            if *count > 2
                || !bounded_text(&receptor.description, 1024)
                || receptor.embedding_profile == [0; 32]
                || receptor.embedding_profile != primary.embedding_profile
                || !(1..=4096).contains(&receptor.vector.len())
                || receptor.vector.len() != primary.vector.len()
                || receptor.vector.iter().any(|value| !value.is_finite())
                || receptor.vector.iter().all(|value| *value == 0.0)
            {
                return Err(invalid("Observation receptors"));
            }
        }
        if receptors.len() != 4 {
            return Err(invalid("Observation receptor categories"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationPolicy {
    pub version: u32,
    pub general_route: [u8; 32],
    pub decision_route: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObservationPendingReason {
    NewEvidence,
    ChangedSupport,
    UpstreamReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObservationWithdrawalReason {
    Refuted,
    LostSupport,
    UnresolvedEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObservationChange {
    Equivalent,
    Qualification,
    Correction,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ObservationActivity {
    pub turn: u64,
    pub source: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObservationComparisonOutcome {
    NoMaterialEffect,
    WeakOrUncertain,
    MaterialChallenge,
    MaterialQualification,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationComparison {
    pub id: ObservationId,
    pub target_revision: u64,
    pub citation: ObservationCitation,
    pub outcome: ObservationComparisonOutcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObservationOperation {
    Create {
        draft: ObservationDraft,
    },
    Pending {
        id: ObservationId,
        expected_revision: u64,
        reason: ObservationPendingReason,
        evidence: Vec<ObservationCitation>,
    },
    Reaffirm {
        id: ObservationId,
        expected_revision: u64,
        reviewed: Vec<ObservationCitation>,
    },
    Successor {
        id: ObservationId,
        expected_revision: u64,
        draft: ObservationDraft,
        change: ObservationChange,
        credited_activity: Option<ObservationActivity>,
    },
    Withdraw {
        id: ObservationId,
        expected_revision: u64,
        reason: ObservationWithdrawalReason,
        reviewed: Vec<ObservationCitation>,
    },
    Comparison {
        comparison: ObservationComparison,
    },
    SemanticAward {
        id: ObservationId,
        expected_revision: u64,
        activity: ObservationActivity,
        evidence: ObservationCitation,
        amount: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationClaimVector {
    pub citation: ObservationCitation,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationClaimVectors {
    pub profile: [u8; 32],
    pub rows: Vec<ObservationClaimVector>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationAuditProgress {
    pub previous: Option<[u8; 32]>,
    pub fingerprint: [u8; 32],
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationBatch {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub input_fingerprint: [u8; 32],
    pub policy: ObservationPolicy,
    pub operations: Vec<ObservationOperation>,
    pub progress: Option<ObservationAuditProgress>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationRevision {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub id: ObservationId,
    pub revision: u64,
    pub predecessor: Option<u64>,
    pub draft: ObservationDraft,
    pub lineage: Vec<ObservationMemoryLineage>,
    pub policy: ObservationPolicy,
    pub input_fingerprint: [u8; 32],
    pub publication_version: u64,
}

impl ObservationRevision {
    pub fn full_citation(&self) -> ObservationCitation {
        ObservationCitation {
            owner_id: self.owner_id,
            history_generation: self.history_generation,
            source: ObservationSource::Observation(self.id),
            revision: self.revision,
            body_id: self.draft.body_id(),
            field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: self.draft.proposition.len() as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObservationStanding {
    Current,
    Pending(ObservationPendingReason),
    Withdrawn(ObservationWithdrawalReason),
    Superseded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObservationInspection<'a> {
    pub revision: &'a ObservationRevision,
    pub standing: ObservationStanding,
    pub triggering_evidence: Vec<ObservationCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationAwardReceipt {
    pub id: ObservationId,
    pub activity: ObservationActivity,
    pub previous: u8,
    pub total: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationBatchReceipt {
    pub publication_version: u64,
    pub input_fingerprint: [u8; 32],
    pub revisions: Vec<(ObservationId, u64)>,
    pub awards: Vec<ObservationAwardReceipt>,
}

fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

fn bounded_text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.trim().is_empty() && !value.contains('\0')
}

fn valid_reviewed(values: &[ObservationCitation]) -> bool {
    values.len() <= MAX_REVIEWED && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn invalid(reason: &'static str) -> PerceptionError {
    PerceptionError::InvalidField(reason)
}
