//! Admit one authenticated zero-link result under the existing owner lease.
//! A single evaluated-pair record is its whole durable effect. Other Dream
//! effects require the later grouped, crash-recoverable publication protocol.
use super::*;
use crate::records::{DreamMemoryCut, DreamNoLinkCut};
use reliquary_memory_core::PayloadState;

pub(super) fn validate_memory(
    common: &CommonState,
    observed: DreamMemoryCut,
) -> Result<(), OwnerSessionError> {
    let history = common
        .memories
        .history(observed.id)
        .map_err(|_| OwnerSessionError::StaleDreamCut)?;
    let current = history
        .revisions
        .last()
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    if history.tombstone.is_some()
        || current.record.id != observed.id
        || current.record.revision != observed.revision
        || current.record.body_id != observed.body_id
        || current.record.archived
        || !matches!(current.payload, PayloadState::Available(_))
    {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    Ok(())
}

fn validate_cut(
    common: &CommonState,
    owner: [u8; 16],
    cut: DreamNoLinkCut,
) -> Result<(), OwnerSessionError> {
    if cut.owner_id != owner {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    if cut.source.id == cut.candidate.id || common.graph.graph_version() != cut.graph_version {
        return Err(OwnerSessionError::StaleDreamCut);
    }
    validate_memory(common, cut.source)?;
    validate_memory(common, cut.candidate)?;
    Ok(())
}

impl RelWriter {
    /// Accept the zero-relation result after Application validates classifier
    /// evidence. Rechecks the exact owner cut before appending one pair marker.
    pub fn commit_dream_no_link(&mut self, cut: DreamNoLinkCut) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        validate_cut(&self.session.common, self.owner_id(), cut)?;
        self.record_dream_pair(cut.source.id, cut.candidate.id)
    }
}

impl PhyWriter {
    /// Same owner-only, one-frame zero-link publication for the private PHY.
    pub fn commit_dream_no_link(&mut self, cut: DreamNoLinkCut) -> Result<bool, OwnerSessionError> {
        validate_cut(&self.common, self.owner_id, cut)?;
        self.record_dream_pair(cut.source.id, cut.candidate.id)
    }
}
