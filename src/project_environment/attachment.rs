use super::{PreparedProjectAttachment, git_upload, lore_upload, scope::ValidatedProjectScope};
use crate::ProjectRepositoryKind;
use std::path::Path;
pub(crate) fn prepare_upload(
    scope: &ValidatedProjectScope,
    source: &Path,
) -> Result<PreparedProjectAttachment, String> {
    scope.revalidate()?;
    match scope.repository_kind() {
        ProjectRepositoryKind::Lore => lore_upload::prepare_upload(scope, source),
        ProjectRepositoryKind::Git => git_upload::prepare_upload(scope, source),
    }
}
