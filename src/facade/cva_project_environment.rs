use crate::{Cva, ProjectAdoptionSource, ProjectRepositoryMode, ProjectRevisionRef};
use std::path::Path;

/// Creates a project REL and publishes the repository association at the
/// upper owner lifecycle boundary.
pub(crate) fn create_project_rel(
    rel_path: &Path,
    project_dir: &Path,
    mode: ProjectRepositoryMode,
) -> Result<Cva, String> {
    let repository = crate::project_environment::prepare_project_repository(project_dir, mode)?;
    let mut cva = match Cva::create_project(rel_path) {
        Ok(cva) => cva,
        Err(error) => {
            repository.rollback();
            return Err(error.to_string());
        }
    };
    if let Err(error) = cva.correlate_project_revision_with_management(
        repository.revision().clone(),
        repository.management(),
    ) {
        drop(cva);
        let _ = std::fs::remove_file(rel_path);
        repository.rollback();
        return Err(error.to_string());
    }
    if let Err(error) = cva.sync() {
        drop(cva);
        let _ = std::fs::remove_file(rel_path);
        repository.rollback();
        return Err(error.to_string());
    }
    let actual = cva
        .latest_project_revision_correlation()
        .ok_or_else(|| "Project REL has no repository correlation after bootstrap".to_string())?;
    repository.validate_correlation(&actual)?;
    Ok(cva)
}

pub(crate) fn inspect_project_adoption_source(
    rel_path: &Path,
) -> Result<ProjectAdoptionSource, String> {
    if !rel_path.is_file() {
        return Err(format!(
            "Project adoption source does not exist: {}",
            rel_path.display()
        ));
    }
    let cva = Cva::open_read_only(rel_path).map_err(|error| format!("invalid REL: {error}"))?;
    let owner_id = cva
        .owner_id()
        .ok_or_else(|| "legacy pre-owner REL/CVA adoption is no longer supported".to_string())?;
    if cva.latest_project_revision_correlation().is_some() {
        return Err(
            "Project REL already has repository correlation; open it instead of adopting it".into(),
        );
    }
    Ok(ProjectAdoptionSource {
        path: rel_path.to_path_buf(),
        owner_id,
    })
}

pub(crate) fn adopt_project_rel(
    source: &ProjectAdoptionSource,
    project_dir: &Path,
    mode: ProjectRepositoryMode,
) -> Result<Cva, String> {
    let repository = crate::project_environment::prepare_project_repository(project_dir, mode)?;
    let mut cva = match Cva::open(&source.path) {
        Ok(cva) => cva,
        Err(error) => {
            repository.rollback();
            return Err(error.to_string());
        }
    };
    if cva.owner_id().as_deref() != Some(source.owner_id.as_str()) {
        repository.rollback();
        return Err("Project adoption source changed durable owner identity".into());
    }
    if cva.latest_project_revision_correlation().is_some() {
        repository.rollback();
        return Err("Project adoption source acquired repository correlation".into());
    }
    if let Err(error) = cva.correlate_project_revision_with_management(
        repository.revision().clone(),
        repository.management(),
    ) {
        repository.rollback();
        return Err(error.to_string());
    }
    if let Err(error) = cva.sync() {
        repository.rollback();
        return Err(error.to_string());
    }
    let actual = cva
        .latest_project_revision_correlation()
        .ok_or_else(|| "Project REL has no repository correlation after adoption".to_string())?;
    repository.validate_correlation(&actual)?;
    Ok(cva)
}

pub(crate) fn validate_project_environment_cva(
    cva: &mut Cva,
    project_dir: &Path,
) -> Result<ProjectRevisionRef, String> {
    let correlation = cva
        .latest_project_revision_correlation()
        .ok_or_else(|| "Project REL has no repository correlation".to_string())?;
    crate::project_environment::current_revision_for(project_dir, &correlation)
}
