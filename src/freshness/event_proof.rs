//! Immutable Freshness journal cuts and graph proof coordinates.

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
