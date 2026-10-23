use crate::cva_reconcile_archive::ArchiveReplayRecord;
use crate::{ArchiveError, Cva, CvaReconcileConflict, CvaReconcileError, Memory, MemoryError};

pub(crate) fn archive_replay_error(
    destination: &Cva,
    record: &ArchiveReplayRecord,
    error: ArchiveError,
) -> CvaReconcileError {
    crate::maintenance::archive_replay_error::archive_replay_error(
        &destination.archive,
        record,
        error,
    )
}

pub(crate) fn memory_replay_error(
    destination: &mut Cva,
    memory: &Memory,
    error: MemoryError,
) -> CvaReconcileError {
    let conflict = match error {
        MemoryError::RevisionConflict => {
            let current_revision = destination
                .memory(memory.id)
                .map(|current| current.revision)
                .unwrap_or(0);
            Some(CvaReconcileConflict::MemoryRevision {
                memory_id: memory.id,
                expected_revision: memory.revision.saturating_sub(1),
                current_revision,
                incoming_revision: memory.revision,
            })
        }
        MemoryError::MutationConflict => Some(CvaReconcileConflict::MemoryMutation {
            memory_id: memory.id,
            mutation_id: memory.mutation_id.clone(),
            incoming_revision: memory.revision,
        }),
        MemoryError::SemanticMutation => Some(CvaReconcileConflict::MemorySemanticMutation {
            memory_id: memory.id,
            incoming_revision: memory.revision,
        }),
        other => return CvaReconcileError::Memories(other),
    };
    CvaReconcileError::Conflict(conflict.expect("mapped Memory conflict"))
}
