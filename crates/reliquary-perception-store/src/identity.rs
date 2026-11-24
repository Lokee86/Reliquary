use crate::{EntityId, MemoryEntityMentionKey, MemoryTextField, RelationshipId};
use serde::{Deserialize, Serialize};

pub const MAX_IDENTITY_RELATIONSHIPS_PER_SIDE: usize = 4;
pub const MAX_IDENTITY_MENTIONS_PER_SIDE: usize = 4;
pub const MAX_IDENTITY_MENTION_QUOTE_BYTES: usize = 512;
pub const MAX_IDENTITY_MENTION_CONTEXT_WORDS: usize = 12;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<IdentityMentionRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityMentionRef {
    pub key: MemoryEntityMentionKey,
    /// Revision of the resolution that bound this span to the endpoint.
    pub revision: u32,
    pub memory: IdentityMemoryRef,
    /// Source ordinal for a typed resolution retained by native conversion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_ordinal: Option<u64>,
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
#[serde(untagged, deny_unknown_fields)]
pub enum IdentityCitation {
    Relationship {
        side: IdentitySide,
        relationship: u16,
        memory: u16,
    },
    Mention {
        side: IdentitySide,
        mention: u16,
    },
}

impl IdentityCitation {
    pub fn side(self) -> IdentitySide {
        match self {
            Self::Relationship { side, .. } | Self::Mention { side, .. } => side,
        }
    }
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
