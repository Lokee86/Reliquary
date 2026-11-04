//! Read-only access to repository-owned project history.
//!
//! Git remains authoritative for repository identity, revisions, trees, and file
//! bytes. This crate validates an existing association and returns bytes from an
//! exact Git tree; it does not create or persist project history.

use reliquary_archive::{ProjectFileRef, ProjectRepositoryKind, validate_project_file_reference};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const REPOSITORY_ID_KEY: &str = "warlock.repositoryId";

/// Resolve bytes for an Archive-owned immutable project reference. Archive
/// records provenance only; Git remains authoritative for identity and bytes.
pub fn read_project_file_reference(
    project_dir: &Path,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    validate_project_file_reference(reference).map_err(|error| error.to_string())?;
    let repository = &reference.revision.repository;
    if repository.kind != ProjectRepositoryKind::Git {
        return Err("Project file reference is not backed by Git".into());
    }
    read_git_revision_file(
        project_dir,
        &repository.repository_id,
        &repository.project_path,
        &reference.revision.revision,
        &reference.path,
        reference.content_hash,
    )
}

/// Read a project file from its exact Git tree revision.
///
/// `repository_id`, `project_path`, `revision`, `path`, and `content_hash` are
/// fields from the caller's Archive-owned project reference. This function
/// does not create a Git repository or assign a missing repository identity.
pub fn read_git_revision_file(
    project_dir: &Path,
    repository_id: &str,
    project_path: &str,
    revision: &str,
    path: &str,
    content_hash: Option<[u8; 32]>,
) -> Result<Vec<u8>, String> {
    let root = git_root(project_dir)?;
    let actual_repository_id = local_repository_id(&root)?;
    if actual_repository_id != repository_id {
        return Err("Project file repository identity does not match the open Git project".into());
    }

    validate_project_path(project_path)?;
    validate_file_path(project_path, path)?;
    let tree = resolve_tree(&root, revision)?;
    let object = format!("{tree}:{path}");
    let bytes = git_bytes(&root, &["cat-file", "blob", &object])?;
    if let Some(expected) = content_hash
        && Sha256::digest(&bytes).as_slice() != expected
    {
        return Err("Historical Git project file content hash mismatch".into());
    }
    Ok(bytes)
}

fn git_root(project_dir: &Path) -> Result<PathBuf, String> {
    if !project_dir.is_dir() {
        return Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ));
    }
    let output = git_output(project_dir, &["rev-parse", "--show-toplevel"])?;
    let raw = String::from_utf8(output.stdout)
        .map_err(|_| "Git repository root is not UTF-8".to_string())?;
    fs::canonicalize(raw.trim()).map_err(|error| format!("failed to resolve Git root: {error}"))
}

fn local_repository_id(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--local", "--get", REPOSITORY_ID_KEY])
        .output()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if !output.status.success() {
        if output.status.code() == Some(1) {
            return Err(
                "Git repository is missing its Warlock repository identity; reopen or re-associate the project explicitly"
                    .into(),
            );
        }
        return Err(git_error("git config --local --get", &output));
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|_| "Git repository identity is not UTF-8".to_string())?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err("Git Warlock repository identity is empty".into());
    }
    Ok(value)
}

fn resolve_tree(root: &Path, revision: &str) -> Result<String, String> {
    // Project Environment records full Git object IDs (commit IDs for normal
    // heads and tree IDs for immutable Warlock snapshots). Reject revision
    // expressions and option-like strings; the stored correlation is opaque.
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Historical Git project revision is not a full object ID".into());
    }
    let expression = format!("{revision}^{{tree}}");
    let output = git_output(
        root,
        &["rev-parse", "--verify", "--end-of-options", &expression],
    )?;
    let tree = String::from_utf8(output.stdout)
        .map_err(|_| "Git returned non-UTF-8 tree identity".to_string())?
        .trim()
        .to_owned();
    if !matches!(tree.len(), 40 | 64) || !tree.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Git returned an invalid tree identity".into());
    }
    Ok(tree)
}

fn validate_project_path(project_path: &str) -> Result<(), String> {
    if project_path == "." {
        return Ok(());
    }
    validate_relative_path(project_path)
        .map_err(|_| "Project repository path is not a normalized repository-relative path".into())
}

fn validate_file_path(project_path: &str, path: &str) -> Result<(), String> {
    validate_relative_path(path).map_err(|_| {
        "Project file path is not a normalized repository-relative path".to_string()
    })?;
    if project_path != "."
        && !path
            .strip_prefix(project_path)
            .is_some_and(|suffix| suffix.starts_with('/') && suffix.len() > 1)
    {
        return Err("Project file path is outside the associated project subtree".into());
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), ()> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(());
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(());
    }
    Ok(())
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = git_output(root, args)?;
    Ok(output.stdout)
}

fn git_output(root: &Path, args: &[&str]) -> Result<Output, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(git_error("Git command", &output))
    }
}

fn git_error(operation: &str, output: &Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if detail.is_empty() {
        format!("{operation} failed")
    } else {
        format!("{operation} failed: {detail}")
    }
}

#[cfg(test)]
#[path = "git_history_tests.rs"]
mod tests;
