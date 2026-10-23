use super::git_command::git_owned;
use super::lore_file::sha256;
use super::scope::ValidatedProjectScope;
use crate::{ProjectFileRef, ProjectRepositoryKind};
use std::ffi::OsString;

pub(crate) fn read_git_project_file(
    scope: &ValidatedProjectScope,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    if reference.revision.repository.kind != ProjectRepositoryKind::Git {
        return Err("Project file is not backed by Git".into());
    }
    scope.authorize_historical(reference)?;
    let repository = scope.git_repository()?;
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
