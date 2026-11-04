//! Application authorization gate for a one-record Dream zero-link result.
//! Graph, lifecycle and Freshness effects need the separate grouped V02 path.
use crate::{DreamOrganizationDraft, DreamWorkflowError};
use reliquary_dream::{DreamRelationDirection, DreamRelationKind};
use reliquary_owner_session::{
    DreamMemoryCut, DreamNoLinkCut, OwnerSessionError, PhyWriter, RelWriter,
};

pub enum DreamOwnerWriter<'a> {
    Rel(&'a mut RelWriter),
    Phy(&'a mut PhyWriter),
}

impl DreamOwnerWriter<'_> {
    fn owner_id(&self) -> [u8; 16] {
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

#[cfg(test)]
#[path = "dream_publication/rel/tests.rs"]
mod rel_tests;
#[cfg(test)]
mod tests;
