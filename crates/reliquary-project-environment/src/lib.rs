mod attachment;
mod file;
mod git;
mod git_command;
mod git_file;
mod git_snapshot;
mod git_upload;
mod lore;
mod lore_checkpoint;
mod lore_file;
mod lore_repository;
mod lore_runtime;
mod lore_upload;
mod repository;
mod upload_paths;

use reliquary_archive::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRevisionCorrelation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectRepositoryMode {
    ManagedLore,
    ManualLore,
    Git,
}
impl ProjectRepositoryMode {
    pub fn from_correlation(correlation: &ProjectRevisionCorrelation) -> Self {
        match (
            correlation.project_revision.repository.kind,
            correlation.repository_management,
        ) {
            (ProjectRepositoryKind::Lore, ProjectRepositoryManagement::WarlockManaged) => {
                Self::ManagedLore
            }
            (ProjectRepositoryKind::Lore, ProjectRepositoryManagement::External) => {
                Self::ManualLore
            }
            (ProjectRepositoryKind::Git, _) => Self::Git,
        }
    }
}
#[derive(Debug)]
pub struct PreparedProjectAttachment {
    pub filename: String,
    pub mime_type: Option<String>,
    pub byte_length: u64,
    pub reference: ProjectFileRef,
}
pub use attachment::prepare_upload;
pub use file::read_project_file;
pub use repository::{
    PreparedProjectRepository, checkpoint_manual_lore, current_revision_for,
    prepare_project_repository, repository_root,
};
