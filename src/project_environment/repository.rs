use super::ProjectRepositoryMode;
use super::git::prepare_git_repository;
use super::lore::{
    PreparedLoreRepository, current_revision, initial_checkpoint, prepare_managed_lore,
};
use super::lore_checkpoint;
use crate::{
    ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryRef,
    ProjectRevisionCorrelation, ProjectRevisionRef,
};
use std::path::{Path, PathBuf};

pub(crate) struct PreparedProjectRepository {
    revision: ProjectRevisionRef,
    management: ProjectRepositoryManagement,
    lore: Option<PreparedLoreRepository>,
}

impl PreparedProjectRepository {
    pub(crate) fn revision(&self) -> &ProjectRevisionRef {
        &self.revision
    }

    pub(crate) fn management(&self) -> ProjectRepositoryManagement {
        self.management
    }

    pub(crate) fn validate_correlation(
        &self,
        actual: &ProjectRevisionCorrelation,
    ) -> Result<(), String> {
        if actual.project_revision != self.revision
            || actual.repository_management != self.management
        {
            return Err("Project REL repository correlation does not match bootstrap state".into());
        }
        Ok(())
    }

    pub(crate) fn rollback(&self) {
        if let Some(repository) = &self.lore {
            repository.rollback();
        }
    }
}

pub(crate) fn prepare_project_repository(
    project_dir: &Path,
    mode: ProjectRepositoryMode,
) -> Result<PreparedProjectRepository, String> {
    let canonical = std::fs::canonicalize(project_dir)
        .map_err(|error| format!("failed to resolve project folder: {error}"))?;
    let project_dir = canonical.as_path();
    match mode {
        ProjectRepositoryMode::ManagedLore => {
            prepare_lore(project_dir, ProjectRepositoryManagement::WarlockManaged)
        }
        ProjectRepositoryMode::ManualLore => {
            prepare_lore(project_dir, ProjectRepositoryManagement::External)
        }
        ProjectRepositoryMode::Git => {
            let repository = prepare_git_repository(project_dir)?;
            Ok(PreparedProjectRepository {
                revision: ProjectRevisionRef {
                    repository: ProjectRepositoryRef {
                        kind: ProjectRepositoryKind::Git,
                        repository_id: repository.repository_id,
                        project_path: repository.project_path,
                    },
                    revision: repository.head,
                },
                management: ProjectRepositoryManagement::External,
                lore: None,
            })
        }
    }
}

fn prepare_lore(
    project_dir: &Path,
    management: ProjectRepositoryManagement,
) -> Result<PreparedProjectRepository, String> {
    let repository = prepare_managed_lore(project_dir)?;
    let revision = match initial_checkpoint(&repository, project_dir) {
        Ok(revision) => revision,
        Err(error) => {
            repository.rollback();
            return Err(error);
        }
    };
    let project_path = match repository.project_path(project_dir) {
        Ok(project_path) => project_path,
        Err(error) => {
            repository.rollback();
            return Err(error);
        }
    };
    Ok(PreparedProjectRepository {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: revision.repository_id,
                project_path,
            },
            revision: revision.revision,
        },
        management,
        lore: Some(repository),
    })
}

pub(crate) fn current_revision_for(
    project_dir: &Path,
    correlation: &ProjectRevisionCorrelation,
) -> Result<ProjectRevisionRef, String> {
    let scope = super::ValidatedProjectScope::current(
        project_dir,
        correlation,
        std::slice::from_ref(correlation),
    )?;
    let repository = match correlation.project_revision.repository.kind {
        ProjectRepositoryKind::Lore => {
            let repository = scope.require_lore(correlation)?;
            let current = current_revision(&repository)?;
            (
                ProjectRepositoryKind::Lore,
                current.repository_id,
                current.revision,
            )
        }
        ProjectRepositoryKind::Git => {
            let repository = scope.require_git(correlation)?;
            (
                ProjectRepositoryKind::Git,
                repository.repository_id,
                repository.head,
            )
        }
    };
    Ok(ProjectRevisionRef {
        repository: ProjectRepositoryRef {
            kind: repository.0,
            repository_id: repository.1,
            project_path: scope.project_path().to_string(),
        },
        revision: repository.2,
    })
}

pub(crate) fn repository_root(
    project_dir: &Path,
    correlation: &ProjectRevisionCorrelation,
) -> Result<PathBuf, String> {
    let scope = super::ValidatedProjectScope::current(
        project_dir,
        correlation,
        std::slice::from_ref(correlation),
    )?;
    scope.revalidate_current(correlation)?;
    Ok(scope.repository_root().to_path_buf())
}

pub(crate) fn checkpoint_manual_lore(
    project_dir: &Path,
    correlation: &ProjectRevisionCorrelation,
) -> Result<ProjectRevisionRef, String> {
    let scope = super::ValidatedProjectScope::current(
        project_dir,
        correlation,
        std::slice::from_ref(correlation),
    )?;
    scope.revalidate_current(correlation)?;
    if correlation.project_revision.repository.kind != ProjectRepositoryKind::Lore
        || correlation.repository_management != ProjectRepositoryManagement::External
    {
        return Err(
            "explicit Lore checkpoint is available only for manually managed Lore Projects".into(),
        );
    }
    let revision = lore_checkpoint::checkpoint_prepared(&scope)?;
    if revision.repository.repository_id != correlation.project_revision.repository.repository_id
        || revision.repository.project_path != correlation.project_revision.repository.project_path
    {
        return Err("Lore repository identity or project path changed during checkpoint".into());
    }
    Ok(revision)
}
