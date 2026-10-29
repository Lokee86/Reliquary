use super::{git_file, lore_file};
use reliquary_archive::{ProjectFileRef, ProjectRepositoryKind};
use std::path::Path;

pub fn read_project_file(
    project_dir: &Path,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    match reference.revision.repository.kind {
        ProjectRepositoryKind::Lore => lore_file::read_lore_project_file(project_dir, reference),
        ProjectRepositoryKind::Git => git_file::read_git_project_file(project_dir, reference),
    }
}
