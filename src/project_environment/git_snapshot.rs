use super::ValidatedProjectScope;
use super::git::PreparedGitRepository;
use super::git_command::{git_bytes_with_input, git_owned, git_text, git_with_index};
use std::ffi::OsString;
use std::fs;
use uuid::Uuid;

pub(crate) fn snapshot_file(
    scope: &ValidatedProjectScope,
    path: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let repository = scope.git_repository()?;
    scope.resolve_current_path(path)?;
    let blob = text(git_bytes_with_input(
        &repository.root,
        &["hash-object", "-w", "--stdin"],
        bytes,
    )?)?;
    // Keep Git's replace-by-rename index machinery in backend metadata. Project
    // ancestors remain pinned against reparse changes throughout snapshot capture.
    let name = format!("warlock-index-{}", Uuid::new_v4());
    let raw = git_text(&repository.root, &["rev-parse", "--git-path", &name])?;
    let raw = std::path::PathBuf::from(raw);
    let index = if raw.is_absolute() {
        raw
    } else {
        repository.root.join(raw)
    };
    let result = snapshot_with_index(&repository, path, &blob, &index);
    let _ = fs::remove_file(&index);
    let _ = fs::remove_file(index.with_extension("lock"));
    result
}

fn snapshot_with_index(
    repository: &PreparedGitRepository,
    path: &str,
    blob: &str,
    index: &std::path::Path,
) -> Result<String, String> {
    git_with_index(
        &repository.root,
        &[OsString::from("read-tree"), OsString::from("HEAD")],
        index,
        None,
    )?;
    git_with_index(
        &repository.root,
        &[
            OsString::from("update-index"),
            OsString::from("--add"),
            OsString::from("--cacheinfo"),
            OsString::from("100644"),
            OsString::from(blob),
            OsString::from(path),
        ],
        index,
        None,
    )?;
    let tree = text(git_with_index(
        &repository.root,
        &[OsString::from("write-tree")],
        index,
        None,
    )?)?;
    let reference = format!("refs/warlock/snapshots/{tree}");
    git_owned(
        &repository.root,
        &[
            OsString::from("update-ref"),
            OsString::from(reference),
            OsString::from(&tree),
        ],
    )?;
    Ok(tree)
}

fn text(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes)
        .map(|value| value.trim().to_string())
        .map_err(|_| "Git returned non-UTF-8 object identity".to_string())
}
