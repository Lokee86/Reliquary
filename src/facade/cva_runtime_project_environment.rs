use crate::{
    Cva, ProjectAdoptionSource, ProjectRepositoryMode, ProjectRevisionRef,
    ReliquaryRuntimeHostError,
};
use std::path::Path;

impl crate::GatewayRuntime {
    pub fn create_project_environment_rel(
        &self,
        rel_path: &Path,
        project_dir: &Path,
        mode: ProjectRepositoryMode,
    ) -> Result<Cva, ReliquaryRuntimeHostError> {
        // Serialize creation against Gateway mounts for the duration of bootstrap.
        let _lifecycle = self.reserve_new_rel_path(rel_path)?;
        crate::cva_project_environment::create_project_rel(rel_path, project_dir, mode)
            .map_err(|error| ReliquaryRuntimeHostError::Operation(error))
    }

    pub fn inspect_project_adoption_source(
        &self,
        rel_path: &Path,
    ) -> Result<ProjectAdoptionSource, ReliquaryRuntimeHostError> {
        let _claim = self.admit_unmounted_path(rel_path)?;
        crate::cva_project_environment::inspect_project_adoption_source(rel_path)
            .map_err(|error| ReliquaryRuntimeHostError::Operation(error))
    }

    pub fn adopt_project_environment_rel(
        &self,
        source: &ProjectAdoptionSource,
        project_dir: &Path,
        mode: ProjectRepositoryMode,
    ) -> Result<Cva, ReliquaryRuntimeHostError> {
        let _claim = self.admit_unmounted_path(&source.path)?;
        crate::cva_project_environment::adopt_project_rel(source, project_dir, mode)
            .map_err(|error| ReliquaryRuntimeHostError::Operation(error))
    }

    pub fn validate_project_environment_cva(
        &self,
        cva: &mut Cva,
        project_dir: &Path,
    ) -> Result<ProjectRevisionRef, ReliquaryRuntimeHostError> {
        crate::cva_project_environment::validate_project_environment_cva(cva, project_dir)
            .map_err(|error| ReliquaryRuntimeHostError::Operation(error))
    }
}
