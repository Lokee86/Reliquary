//! Current revision lookup and immutable tree snapshots backed by Git.

use reliquary_archive::{ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

const REPOSITORY_ID_KEY: &str = "warlock.repositoryId";

/// Read the current Git head after correlating it to the repository identity
/// stored in the Archive association. This performs no repository mutation.
pub fn current_git_revision(
    project_dir: &Path,
    expected: &ProjectRepositoryRef,
) -> Result<ProjectRevisionRef, String> {
    if expected.kind != ProjectRepositoryKind::Git {
        return Err("Project association is not backed by Git".into());
    }
    let root = git_root(project_dir)?;
    let repository_id = repository_id(&root)?;
    if repository_id != expected.repository_id {
        return Err("Project repository identity does not match the open association".into());
    }
    let project_path = project_relative_path(&root, project_dir)?;
    let revision = git_text(&root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    if !full_object_id(&revision) {
        return Err("Git returned an invalid current revision".into());
    }
    Ok(ProjectRevisionRef {
        repository: ProjectRepositoryRef {
            kind: ProjectRepositoryKind::Git,
            repository_id,
            project_path,
        },
        revision,
    })
}

/// Store a file blob and create a protected tree snapshot using a private
/// index under Git's own metadata directory. The user's index and HEAD remain
/// untouched. The returned revision is the immutable tree object ID.
pub fn snapshot_git_project_file(
    project_dir: &Path,
    expected: &ProjectRepositoryRef,
    path: &str,
    bytes: &[u8],
) -> Result<ProjectRevisionRef, String> {
    let current = current_git_revision(project_dir, expected)?;
    validate_project_path(&current.repository.project_path)?;
    validate_file_path(&current.repository.project_path, path)?;
    let root = git_root(project_dir)?;
    let blob = git_with_input(&root, &["hash-object", "-w", "--stdin"], bytes)?;
    if !full_object_id(&blob) {
        return Err("Git returned an invalid file blob identity".into());
    }
    let temp_index = temporary_index(&root)?;
    let result = snapshot_with_index(&root, &current.revision, path, &blob, &temp_index);
    let _ = fs::remove_file(&temp_index);
    let _ = fs::remove_file(lock_path(&temp_index));
    let revision = result?;
    Ok(ProjectRevisionRef {
        repository: current.repository,
        revision,
    })
}

fn snapshot_with_index(
    root: &Path,
    head: &str,
    path: &str,
    blob: &str,
    index: &Path,
) -> Result<String, String> {
    git_with_index(root, &["read-tree", head], index)?;
    git_with_index(
        root,
        &["update-index", "--add", "--cacheinfo", "100644", blob, path],
        index,
    )?;
    let tree = git_text_with_index(root, &["write-tree"], index)?;
    if !full_object_id(&tree) {
        return Err("Git returned an invalid snapshot tree identity".into());
    }
    git_text(
        root,
        &[
            "update-ref",
            &format!("refs/reliquary/snapshots/{tree}"),
            &tree,
        ],
    )?;
    Ok(tree)
}

fn temporary_index(root: &Path) -> Result<PathBuf, String> {
    let raw = git_text(root, &["rev-parse", "--git-path", "index"])?;
    let index = resolve_git_path(root, &raw);
    let parent = index
        .parent()
        .ok_or_else(|| "Git index has no parent directory".to_string())?;
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    Ok(plain_windows_path(&parent.join(format!(
        "project-snapshot-{}-{suffix}.index",
        std::process::id()
    ))))
}

fn lock_path(index: &Path) -> PathBuf {
    let mut value = index.as_os_str().to_os_string();
    value.push(".lock");
    PathBuf::from(value)
}

#[cfg(windows)]
fn plain_windows_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = value.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

#[cfg(not(windows))]
fn plain_windows_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn git_root(project_dir: &Path) -> Result<PathBuf, String> {
    if !project_dir.is_dir() {
        return Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ));
    }
    let root = git_text(project_dir, &["rev-parse", "--show-toplevel"])?;
    fs::canonicalize(root).map_err(|error| format!("failed to resolve Git root: {error}"))
}

fn repository_id(root: &Path) -> Result<String, String> {
    let output = git_output(root, &["config", "--local", "--get", REPOSITORY_ID_KEY])?;
    let value = String::from_utf8(output.stdout)
        .map_err(|_| "Git repository identity is not UTF-8".to_string())?
        .trim()
        .to_owned();
    if value.trim().is_empty() {
        return Err("Git repository identity is empty".into());
    }
    Ok(value)
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

fn validate_project_path(path: &str) -> Result<(), String> {
    if path == "." {
        Ok(())
    } else {
        validate_relative_path(path).map_err(|_| "Project repository path is not normalized".into())
    }
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
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .as_bytes()
            .get(0..2)
            .is_some_and(|p| p[0].is_ascii_alphabetic() && p[1] == b':')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        Err(())
    } else {
        Ok(())
    }
}

fn full_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn resolve_git_path(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_output(root, args)?;
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "Git returned non-UTF-8 output".into())
}

fn git_text_with_index(root: &Path, args: &[&str], index: &Path) -> Result<String, String> {
    let output = git_output_with_index(root, args, index)?;
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "Git returned non-UTF-8 output".into())
}

fn git_with_input(root: &Path, args: &[&str], input: &[u8]) -> Result<String, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    use std::io::Write;
    child
        .stdin
        .take()
        .ok_or_else(|| "failed to open Git input".to_string())?
        .write_all(input)
        .map_err(|error| format!("failed to write Git input: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for Git: {error}"))?;
    if !output.status.success() {
        return Err(git_error("Git command", &output));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "Git returned non-UTF-8 object identity".to_string())?
        .trim()
        .to_owned();
    Ok(text)
}

fn git_with_index(root: &Path, args: &[&str], index: &Path) -> Result<(), String> {
    git_output_with_index(root, args, index).map(|_| ())
}

fn git_output_with_index(root: &Path, args: &[&str], index: &Path) -> Result<Output, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_INDEX_FILE", index)
        .output()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(git_error("Git command", &output))
    }
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
#[path = "git_repository_tests.rs"]
mod tests;
