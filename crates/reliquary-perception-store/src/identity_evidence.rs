//! Transient, owner-qualified inputs for one directional identity comparison.

use crate::{
    Entity, EntityId, RelationshipId,
    identity::{
        IdentityCitation, IdentityEntityRevision, IdentityEvidenceRef, IdentityLimit,
        IdentityMemoryRef, IdentityOutcome, IdentityReason, IdentityRelationshipRef, IdentitySide,
    },
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

pub const MAX_ENTITY_MODEL_INPUT_BYTES: usize = 16 * 1024;
pub const MAX_IDENTITY_CITATIONS: usize = 16;

/// Perception and Memory versions followed by Relationship and Memory prune counts.
/// A mount incarnation belongs to the caller's authorization context.
pub type IdentityEvidenceCut = (u64, u64, usize, usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityJudgment {
    pub outcome: IdentityOutcome,
    pub reason: IdentityReason,
    pub citations: Vec<IdentityCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityEntityEvidence {
    pub id: EntityId,
    pub revision: u64,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
}

impl From<&Entity> for IdentityEntityEvidence {
    fn from(entity: &Entity) -> Self {
        Self {
            id: entity.id,
            revision: entity.revision,
            canonical_name: entity.canonical_name.clone(),
            aliases: entity.aliases.clone(),
            kind: entity.kind.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityParticipantEvidence {
    pub entity: IdentityEntityEvidence,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityObservedMemory {
    pub reference: IdentityMemoryRef,
    /// Digests of source coordinates used to detect common provenance.
    pub lineage: Vec<[u8; 32]>,
    pub exclusion: Option<IdentityMemoryExclusion>,
}

impl IdentityObservedMemory {
    pub fn independent_of(&self, other: &Self) -> bool {
        !self.lineage.is_empty()
            && !other.lineage.is_empty()
            && self.lineage.iter().all(|key| !other.lineage.contains(key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum IdentityMemoryExclusion {
    SourceNotDirect,
    BodyAbsent,
    QuoteAbsent,
    QuoteRepeated,
    ParticipantNames,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityRelationshipEvidence {
    pub id: RelationshipId,
    pub revision: u64,
    pub kind: String,
    pub participants: Vec<IdentityParticipantEvidence>,
    pub memories: Vec<IdentityObservedMemory>,
    /// IDs of cited local Memories with no active revision.
    pub inactive_memories: Vec<[u8; 32]>,
    pub quote_digest: [u8; 32],
    /// Present only when at least one current Memory supports an exact span.
    pub quote: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityEvidence {
    pub owner: [u8; 16],
    pub endpoint: IdentityEntityEvidence,
    pub relationships: Vec<IdentityRelationshipEvidence>,
    pub limit: Option<IdentityLimit>,
}

impl IdentityEvidence {
    pub fn references(&self) -> IdentityEvidenceRef {
        IdentityEvidenceRef {
            endpoint_revision: self.endpoint.revision,
            relationships: self
                .relationships
                .iter()
                .map(|relationship| IdentityRelationshipRef {
                    id: relationship.id,
                    revision: relationship.revision,
                    participants: relationship
                        .participants
                        .iter()
                        .map(|p| IdentityEntityRevision {
                            id: p.entity.id,
                            revision: p.entity.revision,
                        })
                        .collect(),
                    memories: relationship.memories.iter().map(|m| m.reference).collect(),
                })
                .collect(),
            limit: self.limit,
        }
    }

    fn model_input(&self) -> serde_json::Value {
        json!({
            "endpoint": {
                "name": self.endpoint.canonical_name,
                "aliases": self.endpoint.aliases,
                "kind": self.endpoint.kind,
            },
            "relationships": self.relationships.iter().map(|r| {
                json!({
                    "kind": r.kind,
                    "participants": r.participants.iter().map(|p| json!({
                        "endpoint": p.entity.id == self.endpoint.id,
                        "name": p.entity.canonical_name,
                        "kind": p.entity.kind,
                        "roles": p.roles,
                    })).collect::<Vec<_>>(),
                    "quote": r.quote,
                    "memories": r.memories.iter().map(|m| json!({
                        "grounded": m.reference.span.is_some(),
                    })).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct IdentityPairEvidence {
    pub left: IdentityEvidence,
    pub right: IdentityEvidence,
    fingerprint: [u8; 32],
    model_input: Option<String>,
}

impl IdentityPairEvidence {
    pub fn new(mut left: IdentityEvidence, mut right: IdentityEvidence) -> Self {
        let model_input = if left.limit.is_some() || right.limit.is_some() {
            None
        } else {
            let input =
                json!({"left": left.model_input(), "right": right.model_input()}).to_string();
            if input.len() > MAX_ENTITY_MODEL_INPUT_BYTES {
                left.limit = Some(IdentityLimit::Budget);
                right.limit = Some(IdentityLimit::Budget);
                None
            } else {
                Some(input)
            }
        };
        let mut hash = Sha256::new();
        hash.update(b"reliquary-cross-rel-entity-identity-v1\0");
        // These structures contain only ordered collections, numeric coordinates and
        // strings. Sending the serialization into the hash keeps only the bounded
        // evidence projection in memory even when its model representation overflows.
        serde_json::to_writer(&mut hash, &(&left, &right))
            .expect("serialize canonical Entity identity input");
        Self {
            left,
            right,
            fingerprint: hash.finalize().into(),
            model_input,
        }
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    pub fn model_input(&self) -> Option<&str> {
        self.model_input.as_deref()
    }

    pub fn side(&self, side: IdentitySide) -> &IdentityEvidence {
        match side {
            IdentitySide::Left => &self.left,
            IdentitySide::Right => &self.right,
        }
    }

    pub fn witness(
        &self,
        citation: IdentityCitation,
    ) -> Option<(&IdentityRelationshipEvidence, &IdentityObservedMemory, &str)> {
        let relationship = self
            .side(citation.side)
            .relationships
            .get(usize::from(citation.relationship))?;
        let memory = relationship.memories.get(usize::from(citation.memory))?;
        memory.reference.span?;
        Some((relationship, memory, relationship.quote.as_deref()?))
    }

    pub fn quote(&self, citation: IdentityCitation) -> Option<&str> {
        self.witness(citation).map(|(_, _, quote)| quote)
    }
}
