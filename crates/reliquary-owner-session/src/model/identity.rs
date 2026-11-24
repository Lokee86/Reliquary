//! Immutable cross-REL Entity identity records.

pub use reliquary_perception_store::identity::*;
use serde::{Deserialize, Serialize};

pub const MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES: usize = 256;

/// Retained identity-bearing Entity history through the reviewed revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityEndpointProof {
    pub history_generation: [u8; 32],
    pub revision: u64,
    pub digest: [u8; 32],
}

/// The independent operator decision recorded in the pair's local REL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySeparationEvent {
    pub pair: IdentityPair,
    pub revision: u64,
    pub predecessor: Option<u64>,
    pub action_id: [u8; 16],
    pub operator_id: String,
    pub action: IdentitySeparationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum IdentitySeparationAction {
    Confirm {
        receipt_revision: u64,
        receipt_fingerprint: [u8; 32],
        left: IdentityEndpointProof,
        right: IdentityEndpointProof,
    },
    Undo {
        target_revision: u64,
    },
}
