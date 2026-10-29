use super::git::{require_git_repository, validate_reference_repository};
use super::git_command::git_owned;
use super::lore_file::sha256;
use reliquary_archive::{ProjectFileRef, ProjectRepositoryKind};
use std::ffi::OsString;
use std::path::Path;

pub(crate) fn read_git_project_file(
    project_dir: &Path,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    if reference.revision.repository.kind != ProjectRepositoryKind::Git {
        return Err("Project file is not backed by Git".into());
    }
    let repository = require_git_repository(project_dir)?;
    validate_reference_repository(&repository, reference)?;
    let object = format!("{}:{}", reference.revision.revision, reference.path);
    let bytes = git_owned(
        &repository.root,
        &[
            OsString::from("cat-file"),
            OsString::from("blob"),
            OsString::from(object),
        ],
    )?;
    if let Some(expected) = reference.content_hash
        && sha256(&bytes) != expected
    {
        return Err("Historical Git project file content hash mismatch".into());
    }
    Ok(bytes)
}
