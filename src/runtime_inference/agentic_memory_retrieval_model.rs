//! Owner-qualified operation results and ephemeral handoff values.
use crate::{GraphDirection, GraphRelationKind, MemoryRef};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgenticMemoryOperation {
    Query,
    Refinement,
    Traversal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticMemoryContent {
    pub memory: MemoryRef,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticMemoryRelationEvidence {
    pub neighbor: MemoryRef,
    pub kind: GraphRelationKind,
    pub direction: GraphDirection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticMemoryAnchorReport {
    pub anchor: MemoryRef,
    pub unseen_neighbors: usize,
    pub declined_neighbors: usize,
    pub relations: Vec<AgenticMemoryRelationEvidence>,
    pub approved: BTreeSet<MemoryRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgenticMemoryOutcome {
    Memories(Vec<AgenticMemoryContent>),
    AlreadyProvided(BTreeSet<MemoryRef>),
    NoMatches,
}

/// Untrusted/model-visible content is distinct from trusted completion evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticMemoryPayload {
    pub operation: AgenticMemoryOperation,
    pub outcome: AgenticMemoryOutcome,
    pub anchors: Vec<AgenticMemoryAnchorReport>,
}

/// Returned to the trusted host, never retained as controller bookkeeping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAgenticMemoryResult {
    pub(crate) delivery_id: String,
    pub(crate) payload: AgenticMemoryPayload,
    pub(crate) delivered: BTreeSet<MemoryRef>,
    pub(crate) owner_cuts: BTreeMap<String, u64>,
}

impl PreparedAgenticMemoryResult {
    pub fn delivery_id(&self) -> &str {
        &self.delivery_id
    }

    pub fn payload(&self) -> &AgenticMemoryPayload {
        &self.payload
    }

    pub fn delivered(&self) -> &BTreeSet<MemoryRef> {
        &self.delivered
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgenticMemoryError {
    InvalidQuery,
    NoActiveQuery,
    StaleOperation,
    InvalidEvaluation,
    InvalidDeliveryIdentity,
    InvalidDecision(String),
    Authorization(String),
    Retrieval(String),
    Decision(String),
    Handoff(String),
}

impl fmt::Display for AgenticMemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEvaluation => write!(
                f,
                "evaluated Memory identities violate operation eligibility"
            ),
            Self::InvalidQuery => write!(f, "agentic Memory query is empty or too large"),
            Self::NoActiveQuery => write!(f, "agentic Memory operation requires an active query"),
            Self::StaleOperation => {
                write!(f, "agentic Memory controller changed during preparation")
            }
            Self::InvalidDeliveryIdentity => write!(f, "agentic Memory delivery identity is empty"),
            Self::InvalidDecision(message) => write!(f, "invalid Memory decision: {message}"),
            Self::Authorization(message) => write!(f, "Memory authorization: {message}"),
            Self::Retrieval(message) => write!(f, "Memory retrieval: {message}"),
            Self::Decision(message) => write!(f, "Memory decision: {message}"),
            Self::Handoff(message) => write!(f, "Memory handoff: {message}"),
        }
    }
}

impl std::error::Error for AgenticMemoryError {}
