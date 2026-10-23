use crate::cva_reconcile::with_replayed_transaction_time;
use crate::cva_reconcile_conflict_map::archive_replay_error;
use crate::{Cva, CvaReconcileError};

pub(crate) use crate::archive_ordered_replay::{
    ArchiveReplayRecord, ArchiveTail, FileMemoryLinkReplay,
};

pub(crate) fn read_archive_tail(
    cva: &mut Cva,
    start_chunk: usize,
) -> Result<ArchiveTail, CvaReconcileError> {
    read_archive_range(cva, start_chunk, usize::MAX)
}

/// Read a finite source interval without moving later epochs ahead of it.
pub(crate) fn read_archive_range(
    cva: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<ArchiveTail, CvaReconcileError> {
    crate::archive_ordered_replay::read_archive_range(
        crate::archive_ordered_replay::ArchiveReplaySource {
            container: &cva.container,
            archive: &cva.archive,
            project_files: &cva.project_files,
        },
        start_chunk,
        end_chunk,
    )
    .map_err(Into::into)
}

pub(crate) fn replay_archive_tail(
    destination: &mut Cva,
    tail: &ArchiveTail,
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive_version();
    for entry in &tail.records {
        let result = crate::archive_ordered_replay::ArchiveReplayWriter {
            container: &mut destination.container,
            archive: &mut destination.archive,
            project_files: &mut destination.project_files,
        }
        .replay(entry);
        match result {
            Ok(()) => {
                if matches!(
                    entry.record,
                    ArchiveReplayRecord::Node(_) | ArchiveReplayRecord::IngestedTurn(_)
                ) {
                    destination
                        .freshness_on_accepted_activity(destination.archive.rel_turn_count());
                }
            }
            Err(crate::archive_ordered_replay::ArchiveReplayWriteError::Archive(error)) => {
                return Err(archive_replay_error(destination, &entry.record, error));
            }
            Err(crate::archive_ordered_replay::ArchiveReplayWriteError::Project(error)) => {
                return Err(crate::owner_lifecycle::OwnerError::from(error).into());
            }
        }
    }
    Ok(destination.archive_version().saturating_sub(before) as usize)
}

pub(crate) fn replay_file_memory_links(
    destination: &mut Cva,
    links: &[FileMemoryLinkReplay],
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive_version();
    for entry in links {
        with_replayed_transaction_time(destination, entry.transaction_time_ns, |destination| {
            destination.link_file_to_memory(entry.link.file_id, entry.link.memory_id)
        })?;
    }
    Ok(destination.archive_version().saturating_sub(before) as usize)
}
