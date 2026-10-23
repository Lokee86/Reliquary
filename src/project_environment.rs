#[path = "project_environment/scope.rs"]
mod scope;
pub(crate) use scope::ValidatedProjectScope;
#[path = "project_environment/attachment.rs"]
mod attachment;
#[path = "project_environment/file.rs"]
mod file;
#[path = "project_environment/git.rs"]
mod git;
#[path = "project_environment/git_command.rs"]
mod git_command;
#[path = "project_environment/git_file.rs"]
mod git_file;
#[path = "project_environment/git_snapshot.rs"]
mod git_snapshot;
#[path = "project_environment/git_upload.rs"]
mod git_upload;
#[path = "project_environment/lore.rs"]
mod lore;
#[path = "project_environment/lore_checkpoint.rs"]
mod lore_checkpoint;
#[path = "project_environment/lore_file.rs"]
mod lore_file;
#[path = "project_environment/lore_repository.rs"]
mod lore_repository;
#[path = "project_environment/lore_runtime.rs"]
mod lore_runtime;
#[path = "project_environment/lore_upload.rs"]
mod lore_upload;
#[path = "project_environment/repository.rs"]
mod repository;
#[path = "project_environment/upload_paths.rs"]
mod upload_paths;
#[path = "project_environment/write_guard.rs"]
mod write_guard;
pub(crate) use write_guard::WriteGuard;

use crate::ProjectFileRef;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectRepositoryMode {
    ManagedLore,
    ManualLore,
    Git,
}

impl ProjectRepositoryMode {
    pub fn from_correlation(correlation: &crate::ProjectRevisionCorrelation) -> Self {
        match (
            correlation.project_revision.repository.kind,
            correlation.repository_management,
        ) {
            (
                crate::ProjectRepositoryKind::Lore,
                crate::ProjectRepositoryManagement::WarlockManaged,
            ) => Self::ManagedLore,
            (crate::ProjectRepositoryKind::Lore, crate::ProjectRepositoryManagement::External) => {
                Self::ManualLore
            }
            (crate::ProjectRepositoryKind::Git, _) => Self::Git,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAdoptionSource {
    pub path: PathBuf,
    pub owner_id: String,
}

#[derive(Debug)]
pub(crate) struct PreparedProjectAttachment {
    pub(crate) filename: String,
    pub(crate) mime_type: Option<String>,
    pub(crate) byte_length: u64,
    pub(crate) reference: ProjectFileRef,
}

pub(crate) use attachment::prepare_upload;
pub(crate) use file::read_project_file;
pub(crate) use repository::{
    checkpoint_manual_lore, current_revision_for, prepare_project_repository, repository_root,
};
