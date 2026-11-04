use crate::FreshnessMemoryId;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessEventProof {
    pub graph_version: u64,
    pub community_generation: Option<u64>,
    pub community_graph_version: Option<u64>,
}

impl FreshnessEventProof {
    pub fn validate(self) -> Result<(), String> {
        if self.community_generation.is_some() != self.community_graph_version.is_some()
            || self
                .community_graph_version
                .is_some_and(|version| version != self.graph_version)
        {
            return Err("invalid Freshness graph/community proof".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessEventProvenance {
    pub producer_kind: u8,
    pub source_id: String,
    pub origin_owner: [u8; 16],
    pub accepted_turn: u64,
    pub policy_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessEvent {
    pub event_id: String,
    pub accepted_turn: u64,
    pub provenance: FreshnessEventProvenance,
    pub proof: FreshnessEventProof,
    pub effects: BTreeMap<FreshnessMemoryId, i16>,
}

/// Inclusive owner REL cut. Same-turn events are ordered by stable event ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessEventCut {
    pub rel_turn: u64,
    pub event_id_inclusive: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessUseReceipt {
    pub use_id: String,
    pub accepted_turn: u64,
    pub sources: Vec<FreshnessMemoryId>,
    pub policy_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FreshnessReplayEntry {
    Birth {
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
        admitted_at_birth: bool,
    },
    Admission {
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
    },
    Event(FreshnessEvent),
    AcceptedUse(FreshnessUseReceipt, FreshnessEventProof),
}
