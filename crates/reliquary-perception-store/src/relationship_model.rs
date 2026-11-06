//! Immutable Relationship participants and revision-level summary identity.
use super::*;
pub const MAX_RELATIONSHIP_PARTICIPANTS: usize = 16;
pub const MAX_RELATIONSHIP_EVIDENCE: usize = 64;
pub const MAX_RELATIONSHIP_SUMMARY_BYTES: usize = 4096;
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct RelationshipId(pub [u8; 32]);
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipEntityRef {
    pub owner_id: String,
    pub entity_id: EntityId,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipParticipant {
    pub entity: RelationshipEntityRef,
    pub role: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipMemoryRef {
    pub owner_id: String,
    pub memory_id: [u8; 32],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipDraft {
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<RelationshipMemoryRef>,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub id: RelationshipId,
    pub revision: u64,
    pub kind: String,
    pub participants: Vec<RelationshipParticipant>,
    pub evidence: Vec<RelationshipMemoryRef>,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
    pub relationship_version: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipTombstone {
    pub id: RelationshipId,
    pub revision: u64,
    pub relationship_version: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipDescriptionPrune {
    pub id: RelationshipId,
    pub revision: u64,
    pub summary_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
}
pub(super) fn relationship_id(mutation: &str) -> RelationshipId {
    let mut h = Sha256::new();
    h.update(b"reliquary-relationship-id-v1\0");
    h.update((mutation.len() as u64).to_le_bytes());
    h.update(mutation.as_bytes());
    RelationshipId(h.finalize().into())
}
pub(super) fn validate_relationship(d: &RelationshipDraft) -> Result<(), PerceptionError> {
    if d.kind.trim().is_empty()
        || d.kind.len() > 128
        || d.summary.len() > MAX_RELATIONSHIP_SUMMARY_BYTES
        || d.mutation_id.trim().is_empty()
        || d.updated_at_ns < d.created_at_ns
        || !(1..=MAX_RELATIONSHIP_PARTICIPANTS).contains(&d.participants.len())
        || d.evidence.len() > MAX_RELATIONSHIP_EVIDENCE
        || d.participants.iter().any(|p| {
            p.entity.owner_id.trim().is_empty()
                || p.entity.owner_id.len() > 256
                || p.role
                    .as_ref()
                    .is_some_and(|r| r.trim().is_empty() || r.len() > 128)
        })
        || d.evidence
            .iter()
            .any(|r| r.owner_id.trim().is_empty() || r.owner_id.len() > 256)
        || d.participants.windows(2).any(|p| {
            (&p[0].entity.owner_id, p[0].entity.entity_id, &p[0].role)
                >= (&p[1].entity.owner_id, p[1].entity.entity_id, &p[1].role)
        })
        || d.evidence
            .windows(2)
            .any(|p| (&p[0].owner_id, p[0].memory_id) >= (&p[1].owner_id, p[1].memory_id))
    {
        return Err(PerceptionError::InvalidField("Relationship draft"));
    }
    Ok(())
}
impl Relationship {
    pub(super) fn as_draft(&self) -> RelationshipDraft {
        RelationshipDraft {
            kind: self.kind.clone(),
            participants: self.participants.clone(),
            evidence: self.evidence.clone(),
            summary: self.summary.clone(),
            mutation_id: self.mutation_id.clone(),
            created_at_ns: self.created_at_ns,
            updated_at_ns: self.updated_at_ns,
        }
    }
}
