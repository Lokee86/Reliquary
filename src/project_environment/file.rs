use super::{git_file, lore_file, scope::ValidatedProjectScope};
use crate::{ProjectFileRef, ProjectRepositoryKind};
pub(crate) fn read_project_file(
    scope: &ValidatedProjectScope,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    scope.revalidate()?;
    scope.authorize_historical(reference)?;
    match reference.revision.repository.kind {
        ProjectRepositoryKind::Lore => lore_file::read_lore_project_file(scope, reference),
        ProjectRepositoryKind::Git => git_file::read_git_project_file(scope, reference),
    }
}
