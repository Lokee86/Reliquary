//! Freshness owner state contracts.
mod policy;
mod score;
mod wire_policy;

mod storage;

pub use policy::*;
pub use score::*;
#[cfg(feature = "oracle-support")]
pub use storage::FreshnessOracleState;
pub use storage::FreshnessRewriteBaseline;
pub use storage::REWRITE_BASELINE_MAGIC;
pub use storage::{AcceptedImportEventEvidence, ImportSemanticScope};
pub use storage::{
    FreshnessEventProvenance, FreshnessReplayEntry, FreshnessStore, ImportEventEvidence,
    ImportEventScope, SemanticActivation, SemanticEventEvidence, SemanticFragmentEvidence,
    is_semantic_producer, semantic_fragment_event_id, semantic_live_event_id,
};
pub use storage::{HistoricalImportFreshnessScope, HistoricalImportFreshnessSettlement};
pub use wire_policy::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessEventProof {
    pub graph_version: u64,
    pub community_generation: Option<u64>,
    pub community_graph_version: Option<u64>,
}

/// A precise inclusive cut through the canonical Freshness ledger.
/// Events at the same REL turn are ordered by their stable event identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessEventCut {
    pub rel_turn: u64,
    pub event_id_inclusive: Option<String>,
    pub graph_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FreshnessMemoryId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FreshnessFragmentId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FreshnessHistoryGeneration(pub [u8; 16]);

impl FreshnessHistoryGeneration {
    pub const ZERO: Self = Self([0; 16]);
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Immutable contribution retained from a live receipt during import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedFreshnessContribution {
    pub recipient: FreshnessMemoryId,
    pub position: u64,
    pub event_id: [u8; 32],
    pub amount: i16,
}

#[cfg(test)]
mod identity_tests;
