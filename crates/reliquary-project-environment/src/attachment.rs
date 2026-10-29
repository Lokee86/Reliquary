use super::{PreparedProjectAttachment, git_upload, lore_upload};
use reliquary_archive::{
    ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRevisionCorrelation,
};
use std::path::Path;

pub fn prepare_upload(
    correlation: &ProjectRevisionCorrelation,
    project_dir: &Path,
    source: &Path,
) -> Result<PreparedProjectAttachment, String> {
    match (
        correlation.project_revision.repository.kind,
        correlation.repository_management,
    ) {
        (ProjectRepositoryKind::Lore, ProjectRepositoryManagement::WarlockManaged) => {
            lore_upload::prepare_upload(project_dir, source)
        }
        (ProjectRepositoryKind::Lore, ProjectRepositoryManagement::External) => {
            lore_upload::prepare_manual_upload(project_dir, source)
        }
        (ProjectRepositoryKind::Git, _) => git_upload::prepare_upload(
            project_dir,
            source,
            &correlation.project_revision.repository,
        ),
    }
}
