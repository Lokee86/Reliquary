//! Sealed, owner-qualified frontier preparation before inference starts.
use crate::*;
use reliquary_owner_session::model::MemoryId;
use reliquary_owner_session::{DreamBeginAdmission, DreamMemoryCut};
#[derive(Clone, Debug)]
pub struct PreparedDreamFrontier {
    owner: [u8; 16],
    graph_version: u64,
    source: DreamMemoryCut,
    candidates: Vec<DreamMemoryCut>,
}
impl PreparedDreamFrontier {
    pub fn source(&self) -> MemoryId {
        self.source.id
    }
    pub(crate) fn begin_native_import(
        self,
        writer: &mut reliquary_owner_session::RelWriter,
        import_id: &str,
        pass: String,
        turn: u64,
    ) -> Result<DreamBeginAdmission, DreamWorkflowError> {
        if writer.owner_id() != self.owner || writer.dream_graph_version() != self.graph_version {
            return Err(DreamWorkflowError::StaleSnapshot);
        }
        writer
            .begin_native_import_dream_frontier(import_id, pass, self.source, self.candidates, turn)
            .map_err(crate::dream_batch_workflow::map_dream_owner_write)
    }

    pub fn candidate_ids(&self) -> Vec<MemoryId> {
        self.candidates.iter().map(|c| c.id).collect()
    }
}
pub fn prepare_explicit_dream_frontier(
    owner: &EntityOwnerSession<'_>,
    authorized: [u8; 16],
    source: MemoryId,
    candidates: &[MemoryId],
) -> Result<PreparedDreamFrontier, DreamWorkflowError> {
    if owner.owner_id() != authorized {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    if candidates.len() > 16 {
        return Err(DreamWorkflowError::CandidateLimit);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut cuts = Vec::new();
    for id in std::iter::once(&source).chain(candidates.iter()) {
        if !seen.insert(*id) {
            return Err(DreamWorkflowError::DuplicateMemory);
        }
        let current = owner
            .current_memory(*id)
            .map_err(|e| DreamWorkflowError::OwnerRead(e.to_string()))?;
        if current.record.archived {
            return Err(DreamWorkflowError::MemoryUnavailable(*id));
        }
        if !matches!(
            current.payload,
            reliquary_owner_session::model::PayloadState::Available(_)
        ) {
            return Err(DreamWorkflowError::MemoryBodyUnavailable(*id));
        }
        cuts.push(DreamMemoryCut {
            id: *id,
            revision: current.record.revision,
            body_id: current.record.body_id,
        });
    }
    Ok(PreparedDreamFrontier {
        owner: authorized,
        graph_version: owner.graph_version(),
        source: cuts[0],
        candidates: cuts[1..].to_vec(),
    })
}
pub fn begin_prepared_dream_frontier(
    writer: &mut DreamOwnerWriter<'_>,
    authorized: [u8; 16],
    pass: &str,
    prepared: &PreparedDreamFrontier,
    turn: u64,
) -> Result<DreamBeginAdmission, DreamWorkflowError> {
    if writer.owner_id() != authorized || prepared.owner != authorized {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    let result = match writer {
        DreamOwnerWriter::Rel(w) => {
            if w.initial_dream_frontier(pass).is_none()
                && w.dream_graph_version() != prepared.graph_version
            {
                return Err(DreamWorkflowError::StaleSnapshot);
            }
            w.begin_initial_dream_frontier(
                pass.into(),
                prepared.source,
                prepared.candidates.clone(),
                turn,
            )
        }
        DreamOwnerWriter::Phy(w) => {
            if w.initial_dream_frontier(pass).is_none()
                && w.dream_graph_version() != prepared.graph_version
            {
                return Err(DreamWorkflowError::StaleSnapshot);
            }
            w.begin_initial_dream_frontier(
                pass.into(),
                prepared.source,
                prepared.candidates.clone(),
                turn,
            )
        }
    };
    result.map_err(|e| DreamWorkflowError::OwnerWrite(e.to_string()))
}
