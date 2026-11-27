//! Bounded joint discovery over an authorized, immutable working set.
//! Application supplies the route, exact source slices and authorized lookups.

mod batch;
mod discovery;

#[cfg(test)]
mod tests;

pub use batch::{Batch, plan_batches, plan_batches_in_mode};
pub use discovery::{
    BatchResult, GraphLink, ListFailure, ListResult, LookupError, LookupPassage, LookupRequest,
    run_batch, run_batch_for_features, run_batch_with_lookup,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, time::Duration};

pub const CONTRACT_VERSION: u32 = 3;
pub const MAX_MEMORIES: usize = 125;
pub const MAX_CANDIDATES_PER_LIST: usize = 4;
pub const MAX_OPTIONAL_PASSAGE_BYTES: usize = 8 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024;
pub const MAX_LOOKUPS: usize = 2;
pub const MAX_PASSAGES_PER_LOOKUP: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OwnerCut {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub frontier: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SourceId {
    Memory([u8; 32]),
    Observation([u8; 32]),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextField {
    Title,
    Content,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SourceCitation {
    pub owner_id: [u8; 16],
    pub history_generation: [u8; 32],
    pub source: SourceId,
    pub revision: u64,
    pub body_id: [u8; 32],
    pub field: TextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Fresh,
    Stale,
    Dormant,
    Settled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    RelFresh,
    PhySettled,
}

impl AuditMode {
    fn accepts(self, passage: &Passage) -> bool {
        match self {
            Self::RelFresh => passage.grade != Grade::Settled,
            Self::PhySettled => {
                passage.grade == Grade::Settled
                    && passage.eligible == Eligibility::EGO
                    && matches!(passage.citation.source, SourceId::Memory(_))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Feature {
    Ego,
    Perception,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Eligibility {
    pub ego: bool,
    pub perception: bool,
}

impl Eligibility {
    pub const NONE: Self = Self {
        ego: false,
        perception: false,
    };
    pub const EGO: Self = Self {
        ego: true,
        perception: false,
    };
    pub const PERCEPTION: Self = Self {
        ego: false,
        perception: true,
    };
    pub const BOTH: Self = Self {
        ego: true,
        perception: true,
    };

    pub fn allows(self, feature: Feature) -> bool {
        match feature {
            Feature::Ego => self.ego,
            Feature::Perception => self.perception,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            ego: self.ego || other.ego,
            perception: self.perception || other.perception,
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            ego: self.ego && other.ego,
            perception: self.perception && other.perception,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Passage {
    pub citation: SourceCitation,
    pub grade: Grade,
    pub eligible: Eligibility,
    pub text: String,
}

impl Passage {
    pub fn allows(&self, feature: Feature) -> bool {
        self.effective_eligibility().allows(feature)
    }

    fn effective_eligibility(&self) -> Eligibility {
        Eligibility {
            ego: self.eligible.ego
                && matches!(self.grade, Grade::Fresh | Grade::Settled)
                && matches!(self.citation.source, SourceId::Memory(_)),
            perception: self.eligible.perception,
        }
    }

    fn valid_at(&self, cut: OwnerCut) -> bool {
        self.citation.owner_id == cut.owner_id
            && self.citation.history_generation == cut.history_generation
            && self.citation.revision > 0
            && self.citation.start_byte < self.citation.end_byte
            && (self.citation.end_byte - self.citation.start_byte) as usize == self.text.len()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct WorkItem {
    pub passage: Passage,
    /// Each selected list must see all slices of this passage before completing its work.
    pub mandatory: Eligibility,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Participant {
    pub owner_id: [u8; 16],
    pub entity_id: [u8; 32],
    pub revision: u64,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchLimits {
    /// Usable route input bytes for the prompt, serialized schema, and serialized payload.
    pub max_input_bytes: usize,
    pub max_response_bytes: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 128 * 1024,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }
}

/// A semantic owner supplies the item schema and validates each candidate against Core slots.
pub trait CandidateContract {
    type Candidate;

    fn instruction(&self) -> &'static str;
    fn candidate_schema(&self) -> Value;
    fn parse_candidate(&self, value: Value, slots: &Slots<'_>) -> Option<Self::Candidate>;
}

pub struct Slots<'a> {
    sources: &'a [WorkItem],
    participants: &'a [Participant],
    feature: Feature,
}

impl Slots<'_> {
    pub fn source(&self, slot: u16) -> Option<&Passage> {
        let passage = &self.sources.get(usize::from(slot))?.passage;
        passage.allows(self.feature).then_some(passage)
    }

    pub fn participant(&self, slot: u16) -> Option<&Participant> {
        if self.feature != Feature::Perception {
            return None;
        }
        self.participants.get(usize::from(slot))
    }
}

/// Error types deliberately omit provider replies, lookup terms, and source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditError {
    InvalidInput,
    InputLimit,
    ContractChanged,
    Endpoint { retry_after: Option<Duration> },
    UnreadableResponse,
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid authorized audit input",
            Self::InputLimit => "audit input exceeds the route limit",
            Self::ContractChanged => "audit contract differs from the planned batch",
            Self::Endpoint { .. } => "audit endpoint unavailable",
            Self::UnreadableResponse => "unreadable audit response",
        })
    }
}

impl std::error::Error for AuditError {}
