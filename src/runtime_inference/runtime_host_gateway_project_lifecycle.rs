use super::{GatewayRuntime, OwnerKind, mount::MountClaim, mount::MountedFileIdentity};
use crate::{Cva, ProjectAdoptionSource, ProjectRepositoryMode, ProjectRevisionRef};
use std::path::Path;

impl GatewayRuntime {
    pub fn create_project_environment_rel(
        &self,
        rel_path: &Path,
        project_dir: &Path,
        mode: ProjectRepositoryMode,
    ) -> Result<Cva, super::ReliquaryRuntimeHostError> {
        let _claim = self.admit_unmounted_path(rel_path)?;
        crate::project_environment::create_project_rel(rel_path, project_dir, mode)
            .map_err(super::operation)
    }
    pub fn inspect_project_adoption_source(
        &self,
        rel_path: &Path,
    ) -> Result<ProjectAdoptionSource, super::ReliquaryRuntimeHostError> {
        let _claim = self.admit_unmounted_path(rel_path)?;
        crate::project_environment::inspect_project_adoption_source(rel_path)
            .map_err(super::operation)
    }
    pub fn adopt_project_environment_rel(
        &self,
        source: &ProjectAdoptionSource,
        project_dir: &Path,
        mode: ProjectRepositoryMode,
    ) -> Result<Cva, super::ReliquaryRuntimeHostError> {
        let _claim = self.admit_unmounted_path(&source.path)?;
        crate::project_environment::adopt_project_rel(source, project_dir, mode)
            .map_err(super::operation)
    }
    pub fn validate_project_environment_cva(
        &self,
        cva: &mut Cva,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, super::ReliquaryRuntimeHostError> {
        let correlation = cva
            .latest_project_revision_correlation()
            .ok_or_else(|| super::operation("Project REL has no repository correlation"))?;
        let current = crate::project_environment::current_revision_for(project_dir, &correlation)
            .map_err(super::operation)?;
        if current.repository.project_path != correlation.project_revision.repository.project_path {
            cva.correlate_project_revision_with_management(
                current.clone(),
                correlation.repository_management,
            )
            .map_err(super::operation)?;
            cva.sync().map_err(super::operation)?;
        }
        Ok(current)
    }
    pub fn reconcile_rel_file(
        &self,
        canonical: impl AsRef<Path>,
    ) -> Result<super::RelReconciliationReport, super::ReliquaryRuntimeHostError> {
        let path = canonical.as_ref();
        let _claim = self.admit_unmounted_path(path)?;
        super::reconcile_rel_file_unmounted(path).map_err(super::operation)
    }
    fn admit_unmounted_path(
        &self,
        path: &Path,
    ) -> Result<MountClaim, super::ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity = MountedFileIdentity::inspect(path, OwnerKind::Rel)?;
        if self.existing_file_mount(&identity)? {
            return Err(super::operation(
                "project lifecycle requires an unmounted REL",
            ));
        }
        MountClaim::acquire(&identity)
    }
}
