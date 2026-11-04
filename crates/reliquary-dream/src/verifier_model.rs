use crate::{DreamPairClassification, DreamRelationKind};
use reliquary_memory_core::MemoryId;

pub const DREAM_VERIFIER_CONTRACT_VERSION: &str = "v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DreamVerificationSignal {
    Yes,
    No,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DreamVerificationVerdict {
    Accept,
    Reject,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamVerificationPolicy {
    pub topical: bool,
    pub factual: bool,
    pub causal: bool,
    pub recurrent: bool,
    pub duplicate_of: bool,
    pub supersedes: bool,
}

impl Default for DreamVerificationPolicy {
    fn default() -> Self {
        Self {
            topical: false,
            factual: false,
            causal: false,
            recurrent: false,
            duplicate_of: true,
            supersedes: true,
        }
    }
}

impl DreamVerificationPolicy {
    pub fn broad_semantic() -> Self {
        Self {
            topical: false,
            factual: true,
            causal: true,
            recurrent: true,
            duplicate_of: true,
            supersedes: true,
        }
    }

    pub fn should_verify(self, relation: DreamRelationKind) -> bool {
        match relation {
            DreamRelationKind::None => false,
            DreamRelationKind::Topical => self.topical,
            DreamRelationKind::Factual => self.factual,
            DreamRelationKind::Causal => self.causal,
            DreamRelationKind::Recurrent => self.recurrent,
            DreamRelationKind::DuplicateOf => self.duplicate_of,
            DreamRelationKind::Supersedes => self.supersedes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamPairVerification {
    pub verifier_model: String,
    pub a: MemoryId,
    pub b: MemoryId,
    pub classification: DreamPairClassification,
    pub relation_supported: DreamVerificationSignal,
    pub direction_supported: DreamVerificationSignal,
    pub evidence_supported: DreamVerificationSignal,
    pub verdict: DreamVerificationVerdict,
}
