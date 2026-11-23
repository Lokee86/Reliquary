//! Transient, owner-qualified inputs for one directional identity comparison.

use crate::{
    Entity, EntityId, RelationshipId,
    identity::{
        IdentityEntityRevision, IdentityEvidenceRef, IdentityLimit, IdentityMemoryRef,
        IdentityRelationshipRef,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Perception and Memory versions followed by Relationship and Memory prune counts.
/// A mount incarnation belongs to the caller's authorization context.
pub type IdentityEvidenceCut = (u64, u64, usize, usize);

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
}

/// Fingerprint the two owner-qualified captures in recording order.
pub fn fingerprint_identity_evidence(
    left: &IdentityEvidence,
    right: &IdentityEvidence,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-cross-rel-entity-identity-v1\0");
    // The typed projection uses ordered collections and primitive fields.
    // Feed them directly into the hash instead of allocating an encoded copy.
    serde_json::to_writer(&mut hash, &(left, right))
        .expect("serialize canonical Entity identity input");
    hash.finalize().into()
}
