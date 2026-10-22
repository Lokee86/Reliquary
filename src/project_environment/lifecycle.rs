use super::{ProjectAdoptionSource, ProjectRepositoryMode, prepare_project_repository};
use crate::Cva;
use std::path::Path;

pub(crate) fn create_project_rel(
    rel_path: &Path,
    project_dir: &Path,
    mode: ProjectRepositoryMode,
) -> Result<Cva, String> {
    let repository = prepare_project_repository(project_dir, mode)?;
    let mut cva = match Cva::create_project(rel_path) {
        Ok(cva) => cva,
        Err(error) => {
            repository.rollback();
            return Err(error.to_string());
        }
    };
    if let Err(error) = repository.correlate(&mut cva) {
        drop(cva);
        let _ = std::fs::remove_file(rel_path);
        repository.rollback();
        return Err(error);
    }
    if let Err(error) = cva.sync() {
        drop(cva);
        let _ = std::fs::remove_file(rel_path);
        repository.rollback();
        return Err(error.to_string());
    }
    repository.validate(&cva)?;
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
    let repository = prepare_project_repository(project_dir, mode)?;
    let mut cva = Cva::open(&source.path).map_err(|error| error.to_string())?;
    if cva.owner_id().as_deref() != Some(source.owner_id.as_str()) {
        repository.rollback();
        return Err("Project adoption source changed durable owner identity".into());
    }
    if cva.latest_project_revision_correlation().is_some() {
        repository.rollback();
        return Err("Project adoption source acquired repository correlation".into());
    }
    if let Err(error) = repository.correlate(&mut cva) {
        repository.rollback();
        return Err(error);
    }
    if let Err(error) = cva.sync() {
        repository.rollback();
        return Err(error.to_string());
    }
    repository.validate(&cva)?;
    Ok(cva)
}
