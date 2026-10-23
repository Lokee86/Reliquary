use crate::{ProjectFileRef, ProjectRepositoryKind, ProjectRevisionCorrelation};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// A repository mount whose selected project subtree is bound to accepted REL history.
///
/// Machine-local paths are retained only for the lifetime of this scope. Durable identity
/// remains the backend kind and repository ID in the accepted correlation.
#[derive(Clone, Debug)]
pub(crate) struct ValidatedProjectScope {
    kind: ProjectRepositoryKind,
    repository_id: String,
    project_path: String,
    repository_root: PathBuf,
    project_dir: PathBuf,
    selected_path: PathBuf,
    current: ProjectRevisionCorrelation,
    history: Vec<ProjectRevisionCorrelation>,
}

impl ValidatedProjectScope {
    pub(crate) fn current(
        project_dir: &Path,
        current: &ProjectRevisionCorrelation,
        history: &[ProjectRevisionCorrelation],
    ) -> Result<Self, String> {
        let accepted = &current.project_revision.repository;
        validate_repository_path(&accepted.project_path)?;
        if history.last() != Some(current) {
            return Err(
                "project scope must be built from the latest accepted REL correlation".into(),
            );
        }

        for record in history {
            let repository = &record.project_revision.repository;
            if repository.kind != accepted.kind
                || repository.repository_id != accepted.repository_id
            {
                return Err("project correlation history changes repository identity".into());
            }
            validate_repository_path(&repository.project_path)?;
        }
        if accepted.repository_id.trim().is_empty() {
            return Err("project repository identity is empty".into());
        }
        let selected_path = project_dir.to_path_buf();
        let project_dir = fs::canonicalize(project_dir)
            .map_err(|error| format!("failed to resolve project folder: {error}"))?;
        if !project_dir.is_dir() {
            return Err("selected project path is not a directory".into());
        }
        let (repository_root, repository_id, project_path) = match accepted.kind {
            ProjectRepositoryKind::Git => {
                let found = super::git::require_git_repository(&project_dir)?;
                (found.root, found.repository_id, found.project_path)
            }
            ProjectRepositoryKind::Lore => {
                let found = super::lore::require_lore_repository(&project_dir)?;
                let state = super::lore::current_revision(&found)?;
                let project_path = found.project_path(&project_dir)?;
                (found.root, state.repository_id, project_path)
            }
        };
        let repository_root = fs::canonicalize(repository_root)
            .map_err(|error| format!("failed to resolve repository root: {error}"))?;
        let project_dir = fs::canonicalize(project_dir)
            .map_err(|error| format!("failed to resolve selected project folder: {error}"))?;
        if !project_dir.starts_with(&repository_root) {
            return Err("selected project folder is outside the repository root".into());
        }
        validate_repository_path(&project_path)?;
        if accepted.repository_id != repository_id {
            return Err(format!(
                "Project repository identity mismatch: REL expects {}, current repository is {}",
                accepted.repository_id, repository_id
            ));
        }
        if accepted.kind != current.project_revision.repository.kind
            || accepted.project_path != project_path
        {
            return Err(
                "selected project does not match its accepted repository correlation".into(),
            );
        }
        Ok(Self {
            kind: accepted.kind,
            repository_id,
            project_path,
            repository_root,
            project_dir,
            selected_path,
            current: current.clone(),
            history: history.to_vec(),
        })
    }

    pub(crate) fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub(crate) fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub(crate) fn project_path(&self) -> &str {
        &self.project_path
    }

    pub(crate) fn repository_kind(&self) -> ProjectRepositoryKind {
        self.kind
    }

    pub(crate) fn revalidate(&self) -> Result<(), String> {
        self.revalidate_current(&self.current)
    }

    /// Re-discovers the backend and canonical mount before each operation.
    pub(crate) fn revalidate_current(
        &self,
        current: &ProjectRevisionCorrelation,
    ) -> Result<(), String> {
        if current != &self.current {
            return Err("project correlation changed after scope validation".into());
        }
        let rebuilt = Self::current(&self.selected_path, current, &self.history)?;
        if rebuilt.kind != self.kind
            || rebuilt.repository_id != self.repository_id
            || rebuilt.project_path != self.project_path
            || rebuilt.repository_root != self.repository_root
            || rebuilt.project_dir != self.project_dir
        {
            return Err(
                "project repository mount or selected subtree changed after validation".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn require_git(
        &self,
        current: &ProjectRevisionCorrelation,
    ) -> Result<super::git::PreparedGitRepository, String> {
        self.require_kind(current, ProjectRepositoryKind::Git)?;
        let repository = super::git::require_git_repository(&self.project_dir)?;
        if repository.root != self.repository_root
            || repository.repository_id != self.repository_id
            || repository.project_path != self.project_path
        {
            return Err("Git repository changed after project scope validation".into());
        }
        Ok(repository)
    }

    pub(crate) fn git_repository(&self) -> Result<super::git::PreparedGitRepository, String> {
        self.require_git(&self.current)
    }

    pub(crate) fn require_lore(
        &self,
        current: &ProjectRevisionCorrelation,
    ) -> Result<super::lore::PreparedLoreRepository, String> {
        self.require_kind(current, ProjectRepositoryKind::Lore)?;
        let repository = super::lore::require_lore_repository(&self.project_dir)?;
        let revision = super::lore::current_revision(&repository)?;
        if repository.root != self.repository_root
            || revision.repository_id != self.repository_id
            || repository.project_path(&self.project_dir)? != self.project_path
        {
            return Err("Lore repository changed after project scope validation".into());
        }
        Ok(repository)
    }

    pub(crate) fn lore_repository(&self) -> Result<super::lore::PreparedLoreRepository, String> {
        self.require_lore(&self.current)
    }

    fn require_kind(
        &self,
        current: &ProjectRevisionCorrelation,
        kind: ProjectRepositoryKind,
    ) -> Result<(), String> {
        self.revalidate_current(current)?;
        if self.kind != kind {
            return Err("validated project scope uses a different backend".into());
        }
        Ok(())
    }

    /// Authorizes a historical reference only when its project association was accepted by
    /// this REL. The reference cannot enroll a new repository path.
    pub(crate) fn authorize_historical(&self, reference: &ProjectFileRef) -> Result<(), String> {
        self.revalidate()?;
        let repository = &reference.revision.repository;
        validate_repository_path(&repository.project_path)?;
        if repository.kind != self.kind || repository.repository_id != self.repository_id {
            return Err(
                "Project file repository identity does not match the validated project".into(),
            );
        }
        let enrolled = self.history.iter().any(|record| {
            let accepted = &record.project_revision.repository;
            accepted.kind == repository.kind
                && accepted.repository_id == repository.repository_id
                && accepted.project_path == repository.project_path
        });
        if !enrolled {
            return Err(
                "Project file refers to a project path not enrolled in this REL history".into(),
            );
        }
        validate_file_path(&repository.project_path, &reference.path)
    }

    pub(crate) fn authorize_file_path(&self, path: &str) -> Result<(), String> {
        validate_file_path(&self.project_path, path)
    }

    /// Resolve a current-operation destination beneath the selected subtree. Existing
    /// ancestors are canonicalized so symlinks cannot redirect a write outside the project.
    pub(crate) fn resolve_current_path(&self, path: &str) -> Result<PathBuf, String> {
        self.revalidate_physical_mount()?;
        validate_file_path(&self.project_path, path)?;
        let relative = Path::new(path);
        let target = self.repository_root.join(relative);
        let resolved = canonicalize_with_missing_tail(&target)?;
        if !resolved.starts_with(&self.project_dir) {
            return Err("project file path escapes the selected project subtree".into());
        }
        Ok(target)
    }

    fn revalidate_physical_mount(&self) -> Result<(), String> {
        let selected = fs::canonicalize(&self.selected_path)
            .map_err(|error| format!("selected project path changed or disappeared: {error}"))?;
        let project = fs::canonicalize(&self.project_dir)
            .map_err(|error| format!("selected project folder changed or disappeared: {error}"))?;
        let root = fs::canonicalize(&self.repository_root)
            .map_err(|error| format!("repository root changed or disappeared: {error}"))?;
        if selected != self.project_dir
            || project != self.project_dir
            || root != self.repository_root
            || !project.starts_with(&root)
        {
            return Err("project repository mount changed after validation".into());
        }
        Ok(())
    }
}

pub(super) fn validate_repository_path(path: &str) -> Result<(), String> {
    if path == "." {
        return Ok(());
    }
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.contains('\0')
        || path.split('/').any(|component| {
            component.is_empty()
                || matches!(component, "." | "..")
                || component.ends_with('.')
                || component.ends_with(' ')
                || reserved_component(component)
        })
    {
        return Err("project path must be a normalized repository-relative path".into());
    }
    Ok(())
}

fn validate_file_path(project_path: &str, path: &str) -> Result<(), String> {
    validate_repository_path(project_path)?;
    validate_repository_path(path)?;
    if path == "." {
        return Err("project file path must name a file beneath the project".into());
    }
    let project_components = path_components(project_path);
    let file_components = path_components(path);
    if !file_components.starts_with(&project_components)
        || file_components.len() <= project_components.len()
    {
        return Err("project file path is outside the accepted project subtree".into());
    }
    if path
        .split('/')
        .any(|component| reserved_component(component))
    {
        return Err("project file path targets reserved semantic state".into());
    }
    Ok(())
}

fn path_components(path: &str) -> Vec<&str> {
    if path == "." {
        Vec::new()
    } else {
        path.split('/').collect()
    }
}

fn reserved_component(component: &str) -> bool {
    [".git", ".lore", ".urc", ".warlock"]
        .iter()
        .any(|reserved| component.eq_ignore_ascii_case(reserved))
}

fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf, String> {
    let mut tail = Vec::new();
    let mut cursor = path;
    loop {
        match fs::symlink_metadata(cursor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to inspect project path: {error}")),
        }
        let name = cursor
            .file_name()
            .ok_or_else(|| "project path has no existing ancestor".to_string())?;
        tail.push(name.to_os_string());
        cursor = cursor
            .parent()
            .ok_or_else(|| "project path has no existing ancestor".to_string())?;
    }
    let mut resolved = fs::canonicalize(cursor)
        .map_err(|error| format!("failed to resolve project path ancestor: {error}"))?;
    for component in tail.into_iter().rev() {
        if matches!(
            Path::new(&component).components().next(),
            Some(Component::ParentDir | Component::RootDir | Component::Prefix(_))
        ) {
            return Err("project path contains an invalid component".into());
        }
        resolved.push(component);
    }
    Ok(resolved)
}
