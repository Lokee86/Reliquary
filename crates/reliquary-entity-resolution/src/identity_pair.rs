//! Pair-specific input and budget for the Entity identity evaluator.

use crate::MAX_ENTITY_MODEL_INPUT_BYTES;
use reliquary_perception_store::{
    identity::{IdentityCitation, IdentityLimit, IdentityOutcome, IdentityReason, IdentitySide},
    identity_evidence::{
        IdentityEvidence, IdentityObservedMemory, IdentityRelationshipEvidence,
        fingerprint_identity_evidence,
    },
};
use serde_json::{Value, json};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityJudgment {
    pub outcome: IdentityOutcome,
    pub reason: IdentityReason,
    pub citations: Vec<IdentityCitation>,
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
                json!({"left": model_input(&left), "right": model_input(&right)}).to_string();
            if input.len() > MAX_ENTITY_MODEL_INPUT_BYTES {
                left.limit = Some(IdentityLimit::Budget);
                right.limit = Some(IdentityLimit::Budget);
                None
            } else {
                Some(input)
            }
        };
        let fingerprint = fingerprint_identity_evidence(&left, &right);
        Self {
            left,
            right,
            fingerprint,
            model_input,
        }
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    pub(crate) fn model_input(&self) -> Option<&str> {
        self.model_input.as_deref()
    }

    pub(crate) fn side(&self, side: IdentitySide) -> &IdentityEvidence {
        match side {
            IdentitySide::Left => &self.left,
            IdentitySide::Right => &self.right,
        }
    }

    pub(crate) fn witness(
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

fn model_input(side: &IdentityEvidence) -> Value {
    json!({
        "endpoint": {
            "name": side.endpoint.canonical_name,
            "aliases": side.endpoint.aliases,
            "kind": side.endpoint.kind,
        },
        "relationships": side.relationships.iter().map(|r| {
            json!({
                "kind": r.kind,
                "participants": r.participants.iter().map(|p| json!({
                    "endpoint": p.entity.id == side.endpoint.id,
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
