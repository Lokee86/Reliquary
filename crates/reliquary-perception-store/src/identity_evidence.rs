//! Transient, owner-qualified inputs for one directional identity comparison.

use crate::{
    Entity, EntityId, MemoryEntityMentionKey, RelationshipId,
    identity::{
        IdentityEntityRevision, IdentityEvidenceRef, IdentityLimit, IdentityMemoryRef,
        IdentityMentionRef, IdentityRelationshipRef, MAX_IDENTITY_MENTION_CONTEXT_WORDS,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Perception and Memory versions, Relationship and Memory prune counts, and retained Resolution history length.
/// A mount incarnation belongs to the caller's authorization context.
pub type IdentityEvidenceCut = (u64, u64, usize, usize, usize);

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
pub struct IdentityMentionEvidence {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub memory: IdentityObservedMemory,
    pub quote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_ordinal: Option<u64>,
}

impl IdentityMentionEvidence {
    /// The exact resolved surface inside the retained source excerpt.
    pub fn surface(&self) -> Option<&str> {
        let span = self.memory.reference.span?;
        if span.field != self.key.field {
            return None;
        }
        let start = self.key.start_byte.checked_sub(span.start_byte)? as usize;
        let end = self.key.end_byte.checked_sub(span.start_byte)? as usize;
        self.quote.get(start..end).filter(|value| !value.is_empty())
    }

    /// Rank nearby context while ignoring the resolved name and other Entity labels.
    pub fn context_score(&self, endpoint: &IdentityEntityEvidence) -> usize {
        self.context_score_for(endpoint, &self.quote)
    }

    pub fn context_score_for(&self, endpoint: &IdentityEntityEvidence, excerpt: &str) -> usize {
        let Some(surface) = self.surface() else {
            return 0;
        };
        let labels = std::iter::once(endpoint.canonical_name.as_str())
            .chain(endpoint.aliases.iter().map(String::as_str))
            .chain([surface, endpoint.kind.as_str()])
            .flat_map(normalized_words)
            .collect::<BTreeSet<_>>();
        normalized_words(excerpt)
            .filter(|word| !labels.contains(word))
            .take(MAX_IDENTITY_MENTION_CONTEXT_WORDS)
            .count()
    }
}

fn normalized_words(text: &str) -> impl Iterator<Item = String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityEvidence {
    pub owner: [u8; 16],
    pub endpoint: IdentityEntityEvidence,
    pub relationships: Vec<IdentityRelationshipEvidence>,
    pub limit: Option<IdentityLimit>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<IdentityMentionEvidence>,
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
            mentions: self
                .mentions
                .iter()
                .map(|mention| IdentityMentionRef {
                    key: mention.key,
                    revision: mention.revision,
                    memory: mention.memory.reference,
                    retained_ordinal: mention.retained_ordinal,
                })
                .collect(),
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
