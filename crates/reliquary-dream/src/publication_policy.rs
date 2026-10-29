use crate::{
    DreamEvidenceSide, DreamPairClassification, DreamPairVerification, DreamRelationDirection,
    DreamRelationKind, DreamVerificationPolicy, DreamVerificationVerdict,
};
use reliquary_graph::GraphRelationKind;
use reliquary_memory_core::MemoryId;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPolicyError {
    InvalidClassification,
    VerificationMismatch,
    MissingVerification,
}

type RelationIdentity = (MemoryId, MemoryId, GraphRelationKind);

pub fn verification_gate(
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
) -> Result<Option<DreamVerificationVerdict>, PublicationPolicyError> {
    if let Some(verification) = verification {
        if verification.a != classification.a
            || verification.b != classification.b
            || verification.classification != *classification
        {
            return Err(PublicationPolicyError::VerificationMismatch);
        }
        return Ok(match verification.verdict {
            DreamVerificationVerdict::Accept => None,
            verdict => Some(verdict),
        });
    }
    if policy.should_verify(classification.relation) {
        Err(PublicationPolicyError::MissingVerification)
    } else {
        Ok(None)
    }
}

pub fn validate_classification(
    classification: &DreamPairClassification,
) -> Result<(), PublicationPolicyError> {
    if classification.a == classification.b {
        return Err(PublicationPolicyError::InvalidClassification);
    }
    let evidence_valid = if classification.relation == DreamRelationKind::None {
        classification.evidence.is_empty()
    } else {
        classification.evidence.len() == 2
            && classification
                .evidence
                .iter()
                .filter(|item| item.side == DreamEvidenceSide::A)
                .count()
                == 1
            && classification
                .evidence
                .iter()
                .filter(|item| item.side == DreamEvidenceSide::B)
                .count()
                == 1
    };
    let valid = evidence_valid
        && match classification.relation {
            DreamRelationKind::None => classification.direction == DreamRelationDirection::None,
            DreamRelationKind::Topical
            | DreamRelationKind::Recurrent
            | DreamRelationKind::DuplicateOf => {
                classification.direction == DreamRelationDirection::Undirected
            }
            DreamRelationKind::Factual
            | DreamRelationKind::Causal
            | DreamRelationKind::Supersedes => matches!(
                classification.direction,
                DreamRelationDirection::AToB | DreamRelationDirection::BToA
            ),
        };
    valid
        .then_some(())
        .ok_or(PublicationPolicyError::InvalidClassification)
}

pub fn desired_relations(
    classification: &DreamPairClassification,
) -> Result<HashSet<RelationIdentity>, PublicationPolicyError> {
    let mut desired = HashSet::new();
    let kind = match classification.relation {
        DreamRelationKind::None => return Ok(desired),
        DreamRelationKind::Topical => GraphRelationKind::Topical,
        DreamRelationKind::Factual => GraphRelationKind::Factual,
        DreamRelationKind::Causal => GraphRelationKind::Causal,
        DreamRelationKind::Recurrent => GraphRelationKind::Recurrent,
        DreamRelationKind::Supersedes => GraphRelationKind::Supersedes,
        DreamRelationKind::DuplicateOf => {
            return Err(PublicationPolicyError::InvalidClassification);
        }
    };
    match classification.direction {
        DreamRelationDirection::Undirected => {
            desired.insert((classification.a, classification.b, kind));
            desired.insert((classification.b, classification.a, kind));
        }
        DreamRelationDirection::AToB => {
            desired.insert((classification.a, classification.b, kind));
        }
        DreamRelationDirection::BToA => {
            desired.insert((classification.b, classification.a, kind));
        }
        DreamRelationDirection::None => return Err(PublicationPolicyError::InvalidClassification),
    }
    Ok(desired)
}
