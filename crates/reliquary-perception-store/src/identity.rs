use crate::{EntityId, MemoryTextField, RelationshipId};
use serde::{Deserialize, Serialize};

pub const MAX_IDENTITY_RELATIONSHIPS_PER_SIDE: usize = 4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityPair {
    pub local_owner: [u8; 16],
    pub local_entity: EntityId,
    pub peer_owner: [u8; 16],
    pub peer_entity: EntityId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityReceipt {
    pub pair: IdentityPair,
    pub revision: u64,
    pub predecessor: Option<u64>,
    pub fingerprint: [u8; 32],
    pub policy: u32,
    pub route: String,
    pub left: IdentityEvidenceRef,
    pub right: IdentityEvidenceRef,
    pub outcome: IdentityOutcome,
    pub reason: IdentityReason,
    pub citations: Vec<IdentityCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityEvidenceRef {
    pub endpoint_revision: u64,
    pub relationships: Vec<IdentityRelationshipRef>,
    pub limit: Option<IdentityLimit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityRelationshipRef {
    pub id: RelationshipId,
    pub revision: u64,
    pub participants: Vec<IdentityEntityRevision>,
    pub memories: Vec<IdentityMemoryRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityEntityRevision {
    pub id: EntityId,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityMemoryRef {
    pub id: [u8; 32],
    pub revision: u64,
    pub body_id: [u8; 32],
    pub span: Option<IdentitySpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySpan {
    pub field: MemoryTextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityCitation {
    pub side: IdentitySide,
    pub relationship: u16,
    pub memory: u16,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum IdentitySide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IdentityLimit {
    Relationships,
    Budget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IdentityOutcome {
    SameIdentity,
    RelatedDistinct,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IdentityReason {
    Corroborated,
    Distinct,
    InsufficientEvidence,
    Ambiguity,
    Conflict,
    EvidenceLimit,
}
