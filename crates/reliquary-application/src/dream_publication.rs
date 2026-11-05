//! Checked Application admission for zero-link and one positive Dream relation.
//! Lifecycle, first-cycle Freshness and multi-pair publication remain V02 work.
use crate::{DreamOrganizationDraft, DreamWorkflowError};
use reliquary_dream::{DreamRelationDirection, DreamRelationKind, DreamVerificationVerdict};
use reliquary_owner_session::model::{NodeRef, RelationKind, RelationOrigin};
use reliquary_owner_session::{
    DreamMemoryCut, DreamNoLinkCut, DreamRelationCut, OwnerSessionError, PhyWriter, RelWriter,
};

pub enum DreamOwnerWriter<'a> {
    Rel(&'a mut RelWriter),
    Phy(&'a mut PhyWriter),
}

impl DreamOwnerWriter<'_> {
    pub(crate) fn owner_id(&self) -> [u8; 16] {
        match self {
            Self::Rel(writer) => writer.owner_id(),
            Self::Phy(writer) => writer.owner_id(),
        }
    }

    fn commit_no_link(&mut self, cut: DreamNoLinkCut) -> Result<bool, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.commit_dream_no_link(cut),
            Self::Phy(writer) => writer.commit_dream_no_link(cut),
        }
    }

    fn commit_relation(&mut self, cut: DreamRelationCut) -> Result<bool, OwnerSessionError> {
        match self {
            Self::Rel(writer) => writer.commit_dream_grouped_relation(cut),
            Self::Phy(writer) => writer.commit_dream_grouped_relation(cut),
        }
    }
}

/// One durable evaluated-pair marker is the entire accepted effect of a
/// classifier's zero-link decision. Explicitly refuses multi-pair drafts and
/// positive relations rather than attempting a non-atomic sequence of writes.
/// The caller must have authenticated access to the writer before this call.
pub fn commit_dream_zero_link(
    writer: &mut DreamOwnerWriter<'_>,
    authorized_owner: [u8; 16],
    draft: &DreamOrganizationDraft,
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
    if classification.a == classification.b
        || ![source.memory_id, candidate.memory_id].contains(&classification.a)
        || ![source.memory_id, candidate.memory_id].contains(&classification.b)
        || classification.relation != DreamRelationKind::None
        || classification.direction != DreamRelationDirection::None
        || !classification.evidence.is_empty()
        || classification.model.is_empty()
        || !draft.verifications.is_empty()
        || !draft.plan.relation_changes.is_empty()
        || !draft.plan.lifecycle_intents.is_empty()
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let cut = DreamNoLinkCut {
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
    };
    writer.commit_no_link(cut).map_err(|error| match error {
        OwnerSessionError::StaleDreamCut => DreamWorkflowError::StaleSnapshot,
        _ => DreamWorkflowError::OwnerWrite(error.to_string()),
    })
}

/// Admit one positive classifier-derived, evidence-backed Memory relation
/// and its evaluated-pair receipt as a single recoverable owner record.
/// Verified DuplicateOf is supported; Supersedes/lifecycle and multi-pair
/// effects are routed through their separate grouped admission path.
pub fn commit_dream_relation(
    writer: &mut DreamOwnerWriter<'_>,
    authorized_owner: [u8; 16],
    draft: &DreamOrganizationDraft,
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
    let kind = match classification.relation {
        DreamRelationKind::Topical => RelationKind::Topical,
        DreamRelationKind::Factual => RelationKind::Factual,
        DreamRelationKind::Causal => RelationKind::Causal,
        DreamRelationKind::Recurrent => RelationKind::Recurrent,
        DreamRelationKind::DuplicateOf => RelationKind::DuplicateOf,
        _ => return Err(DreamWorkflowError::UnsupportedPublication),
    };
    if classification.a == classification.b
        || ![source.memory_id, candidate.memory_id].contains(&classification.a)
        || ![source.memory_id, candidate.memory_id].contains(&classification.b)
        || classification.direction == DreamRelationDirection::None
        || classification.model.is_empty()
        || classification.evidence.len() != 2
        || change.kind != kind
        || !change.active
        || change.origin != RelationOrigin::Dream
        || !draft.plan.lifecycle_intents.is_empty()
        || draft.verifications.iter().any(|check| {
            check.classification != *classification
                || check.verdict != DreamVerificationVerdict::Accept
        })
        || (kind == RelationKind::DuplicateOf
            && (draft.verifications.len() != 1 || draft.verifications[0].verifier_model.is_empty()))
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let expected = match classification.direction {
        DreamRelationDirection::BToA => (classification.b, classification.a),
        _ => (classification.a, classification.b),
    };
    let expected_endpoints = (NodeRef::Memory(expected.0.0), NodeRef::Memory(expected.1.0));
    let actual_endpoints = (change.source, change.target);
    if actual_endpoints != expected_endpoints
        && !(kind == RelationKind::DuplicateOf
            && actual_endpoints == (expected_endpoints.1, expected_endpoints.0))
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    writer
        .commit_relation(DreamRelationCut {
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
        })
        .map_err(|error| match error {
            OwnerSessionError::StaleDreamCut => DreamWorkflowError::StaleSnapshot,
            _ => DreamWorkflowError::OwnerWrite(error.to_string()),
        })
}

#[cfg(test)]
#[path = "dream_publication/rel/tests.rs"]
mod rel_tests;
#[cfg(test)]
mod tests;
