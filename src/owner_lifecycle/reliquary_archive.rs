//! Archive owner reconstruction and its immediate immutable reference checks.
use super::OwnerError;
use crate::Archive;
use crate::archive_rebuild::ArchiveOpenState;
use crate::project_file_binding_store::ProjectFileStore;

/// Finish Archive before the separate History retention-admission checkpoint.
pub(crate) fn finish_reliquary_archive(
    state: ArchiveOpenState,
    project_files: &ProjectFileStore,
) -> Result<Archive, OwnerError> {
    let archive = state.finish()?;
    archive.validate_references(project_files.accepted_facts())?;
    Ok(archive)
}
