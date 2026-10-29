use super::git_command::{git_failure, git_success, git_text};
use reliquary_archive::ProjectFileRef;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const REPOSITORY_ID_KEY: &str = "warlock.repositoryId";

#[derive(Clone, Debug)]
pub(crate) struct PreparedGitRepository {
    pub(crate) root: PathBuf,
    pub(crate) repository_id: String,
    pub(crate) project_path: String,
    pub(crate) head: String,
}

pub(crate) fn prepare_git_repository(project_dir: &Path) -> Result<PreparedGitRepository, String> {
    let root = discover_root(project_dir)?;
    let head = git_text(&root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let repository_id = match local_repository_id(&root)? {
        Some(id) if !id.trim().is_empty() => id,
        Some(_) => return Err("Git Warlock repository identity is empty".into()),
        None => {
            let id = Uuid::new_v4().to_string();
            git_success(
                &root,
                &["config", "--local", REPOSITORY_ID_KEY, id.as_str()],
            )?;
            id
        }
    };
    let repository = PreparedGitRepository {
        root: root.clone(),
        repository_id,
        project_path: project_relative_path(&root, project_dir)?,
        head,
    };
    maintain_reserved_exclude(&repository)?;
    Ok(repository)
}

pub(crate) fn require_git_repository(project_dir: &Path) -> Result<PreparedGitRepository, String> {
    let root = discover_root(project_dir)?;
    let repository_id = local_repository_id(&root)?.ok_or_else(|| {
        "Git repository is missing its Warlock repository identity; reopen or re-associate the project explicitly"
            .to_string()
    })?;
    if repository_id.trim().is_empty() {
        return Err("Git Warlock repository identity is empty".into());
    }
    let head = git_text(&root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let project_path = project_relative_path(&root, project_dir)?;
    Ok(PreparedGitRepository {
        root,
        repository_id,
        project_path,
        head,
    })
}

pub(crate) fn validate_reference_repository(
    repository: &PreparedGitRepository,
    reference: &ProjectFileRef,
) -> Result<(), String> {
    if repository.repository_id != reference.revision.repository.repository_id {
        return Err("Project file repository identity does not match the open Git project".into());
    }
    Ok(())
}

fn discover_root(project_dir: &Path) -> Result<PathBuf, String> {
    if !project_dir.is_dir() {
        return Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ));
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(project_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if !output.status.success() {
        return Err("Git repository is missing for selected project folder".into());
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|_| "Git repository root is not UTF-8".to_string())?;
    fs::canonicalize(value.trim()).map_err(|error| format!("failed to resolve Git root: {error}"))
}

fn local_repository_id(root: &Path) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--local", "--get", REPOSITORY_ID_KEY])
        .output()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if output.status.success() {
        return String::from_utf8(output.stdout)
            .map(|value| Some(value.trim().to_string()))
            .map_err(|_| "Git repository identity is not UTF-8".to_string());
    }
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    Err(git_failure("git config --local --get", &output.stderr))
}

pub(crate) fn maintain_reserved_exclude(repository: &PreparedGitRepository) -> Result<(), String> {
    let raw = git_text(
        &repository.root,
        &["rev-parse", "--git-path", "info/exclude"],
    )?;
    let path = resolve_git_path(&repository.root, &raw);
    let pattern = if repository.project_path == "." {
        "/.warlock/".to_string()
    } else {
        format!("/{}/.warlock/", repository.project_path)
    };
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    if contents.lines().any(|line| line.trim() == pattern) {
        return Ok(());
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&pattern);
    contents.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, contents).map_err(|error| format!("failed to update Git exclude: {error}"))
}

fn project_relative_path(root: &Path, project_dir: &Path) -> Result<String, String> {
    let project = fs::canonicalize(project_dir)
        .map_err(|error| format!("failed to resolve project folder: {error}"))?;
    let relative = project
        .strip_prefix(root)
        .map_err(|_| "project folder is outside the Git repository".to_string())?;
    let value = relative.to_string_lossy().replace('\\', "/");
    Ok(if value.is_empty() { ".".into() } else { value })
}

fn resolve_git_path(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
