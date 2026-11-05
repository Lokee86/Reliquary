//! Application admission for one verified Supersedes decision and lifecycle.
use crate::{DreamOrganizationDraft, DreamWorkflowError, dream_publication::DreamOwnerWriter};
use reliquary_dream::{DreamRelationDirection, DreamRelationKind, DreamVerificationVerdict};
use reliquary_owner_session::{
    DreamMemoryCut, DreamRelationCut, OwnerSessionError,
    model::{NodeRef, RelationKind, RelationOrigin},
};

impl DreamOwnerWriter<'_> {
    fn commit_supersession(
        &mut self,
        cut: DreamRelationCut,
        accepted_at_ns: i64,
    ) -> Result<bool, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.commit_dream_supersession(cut, accepted_at_ns),
            Self::Phy(writer) => writer.commit_dream_supersession(cut, accepted_at_ns),
        }
    }
}

/// Accept exactly one verified directed supersession. The old Memory's
/// lifecycle update, Graph edge and pair receipt are one owner transaction.
/// The caller supplies an accepted transaction time, reused exactly on retry.
pub fn commit_dream_supersession(
    writer: &mut DreamOwnerWriter<'_>,
    authorized_owner: [u8; 16],
    draft: &DreamOrganizationDraft,
    accepted_at_ns: i64,
) -> Result<bool, DreamWorkflowError> {
    if writer.owner_id() != authorized_owner || draft.owner_id != authorized_owner {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    let [classification] = draft.classifications.as_slice() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    let [source, candidate] = draft.observed_memories.as_slice() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    let [change] = draft.plan.relation_changes.as_slice() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    let [lifecycle] = draft.plan.lifecycle_intents.as_slice() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    let [verification] = draft.verifications.as_slice() else {
        return Err(DreamWorkflowError::UnsupportedPublication);
    };
    if classification.relation != DreamRelationKind::Supersedes
        || !matches!(
            classification.direction,
            DreamRelationDirection::AToB | DreamRelationDirection::BToA
        )
        || classification.a == classification.b
        || classification.model.is_empty()
        || classification.evidence.len() != 2
        || verification.classification != *classification
        || verification.verdict != DreamVerificationVerdict::Accept
        || verification.verifier_model.is_empty()
        || source.memory_id == candidate.memory_id
        || ![source.memory_id, candidate.memory_id].contains(&classification.a)
        || ![source.memory_id, candidate.memory_id].contains(&classification.b)
        || change.kind != RelationKind::Supersedes
        || change.origin != RelationOrigin::Dream
        || !change.active
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let (newer, older) = match classification.direction {
        DreamRelationDirection::AToB => (classification.a, classification.b),
        DreamRelationDirection::BToA => (classification.b, classification.a),
        _ => unreachable!(),
    };
    if change.source != NodeRef::Memory(newer.0)
        || change.target != NodeRef::Memory(older.0)
        || lifecycle.memory_id != older
        || lifecycle.lifecycle_state != "archived"
        || !lifecycle.archived
        || lifecycle.superseded_by != Some(newer)
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    writer
        .commit_supersession(
            DreamRelationCut {
                owner_id: authorized_owner,
                graph_version: draft.graph_version,
                source: DreamMemoryCut {
                    id: source.memory_id,
                    revision: source.revision,
                    body_id: source.body_id,
                },
                candidate: DreamMemoryCut {
                    id: candidate.memory_id,
                    revision: candidate.revision,
                    body_id: candidate.body_id,
                },
                change: *change,
            },
            accepted_at_ns,
        )
        .map_err(|error| match error {
            OwnerSessionError::StaleDreamCut => DreamWorkflowError::StaleSnapshot,
            _ => DreamWorkflowError::OwnerWrite(error.to_string()),
        })
}
