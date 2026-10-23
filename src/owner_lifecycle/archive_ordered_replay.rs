//! Validated cross-owner Archive replay admission over concrete producer parts.
use crate::archive_ordered_replay::{
    ArchiveReplayWriteError, ArchiveReplayWriter, ArchiveTail, FileMemoryLinkReplay,
};
use crate::{Archive, Container, CvaReconcileError};
pub(crate) struct ArchiveReplayDestination<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) archive: &'a mut Archive,
    pub(crate) project_files: &'a mut crate::project_file_binding_store::ProjectFileStore,
}
pub(crate) fn replay_archive_tail(
    destination: ArchiveReplayDestination<'_>,
    tail: &ArchiveTail,
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive.archive_version();
    for entry in &tail.records {
        match (ArchiveReplayWriter {
            container: destination.container,
            archive: destination.archive,
            project_files: destination.project_files,
        })
        .replay(entry)
        {
            Ok(()) => {}
            Err(ArchiveReplayWriteError::Archive(error)) => {
                return Err(super::archive_replay_error::archive_replay_error(
                    destination.archive,
                    &entry.record,
                    error,
                ));
            }
            Err(ArchiveReplayWriteError::Project(error)) => {
                return Err(super::OwnerError::from(error).into());
            }
        }
    }
    Ok(destination.archive.archive_version().saturating_sub(before) as usize)
}
pub(crate) fn replay_file_memory_links(
    container: &mut Container,
    archive: &mut Archive,
    memories: &crate::memory_store::MemoryStore,
    links: &[FileMemoryLinkReplay],
) -> Result<usize, CvaReconcileError> {
    let before = archive.archive_version();
    for entry in links {
        container.set_next_transaction_time_override(entry.transaction_time_ns);
        let result = (|| {
            memories
                .current_body_id(entry.link.memory_id)
                .map_err(|_| crate::ArchiveError::MissingFileMemoryTarget)?;
            archive.link_file_to_memory(container, entry.link.file_id, entry.link.memory_id)
        })();
        container.clear_next_transaction_time_override();
        result?;
    }
    Ok(archive.archive_version().saturating_sub(before) as usize)
}
