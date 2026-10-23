use super::PreparedProjectAttachment;
use super::git_command::{git_bytes_with_input, git_owned};
use super::git_snapshot::snapshot_file;
use super::lore_file::sha256;
use super::scope::ValidatedProjectScope;
use super::upload_paths::{mime_type, upload_destination};
use crate::{ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef};
use std::ffi::OsString;
use std::fs;
use std::path::Path;

pub(crate) fn prepare_upload(
    scope: &ValidatedProjectScope,
    source: &Path,
) -> Result<PreparedProjectAttachment, String> {
    if !source.is_file() {
        return Err(format!("attachment is not a file: {}", source.display()));
    }
    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "attachment filename is not valid UTF-8".to_string())?
        .to_string();
    let bytes = fs::read(source).map_err(|error| format!("failed to read attachment: {error}"))?;
    let repository = scope.git_repository()?;
    let content_hash = sha256(&bytes);
    let byte_length = bytes.len() as u64;

    let (path, revision) = match find_tracked_match(scope, &repository, &bytes)? {
        Some(path) => (path, repository.head.clone()),
        None => {
            let mut destination = upload_destination(scope, &filename, content_hash)?;
            super::upload_paths::materialize(scope, &mut destination, &bytes)?;
            let path = repository_relative(&repository.root, destination.path())?;
            let revision = snapshot_file(scope, &path, &bytes)?;
            (path, revision)
        }
    };

    Ok(PreparedProjectAttachment {
        filename: filename.clone(),
        mime_type: mime_type(&filename),
        byte_length,
        reference: ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Git,
                    repository_id: repository.repository_id,
                    project_path: repository.project_path,
                },
                revision,
            },
            path,
            content_hash: Some(content_hash),
        },
    })
}

fn find_tracked_match(
    scope: &ValidatedProjectScope,
    repository: &super::git::PreparedGitRepository,
    bytes: &[u8],
) -> Result<Option<String>, String> {
    let blob = String::from_utf8(git_bytes_with_input(
        &repository.root,
        &["hash-object", "--stdin"],
        bytes,
    )?)
    .map_err(|_| "Git returned non-UTF-8 blob identity".to_string())?
    .trim()
    .to_string();
    let mut args = vec![
        OsString::from("ls-tree"),
        OsString::from("-r"),
        OsString::from("-z"),
        OsString::from("HEAD"),
    ];
    if repository.project_path != "." {
        args.push(OsString::from("--"));
        args.push(OsString::from(&repository.project_path));
    }
    let tree = git_owned(&repository.root, &args)?;
    for record in tree
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        let header = std::str::from_utf8(&record[..tab]).ok();
        let path = std::str::from_utf8(&record[tab + 1..]).ok();
        let (Some(header), Some(path)) = (header, path) else {
            continue;
        };
        let mut fields = header.split_whitespace();
        let mode = fields.next().unwrap_or_default();
        let kind = fields.next().unwrap_or_default();
        let object = fields.next().unwrap_or_default();
        if kind != "blob" || !matches!(mode, "100644" | "100755") || object != blob {
            continue;
        }
        if path
            .split('/')
            .any(|part| part == ".warlock" || part == ".git")
            || path.starts_with("warlock/generated/")
            || path.contains("/warlock/generated/")
        {
            continue;
        }
        if scope.authorize_file_path(path).is_err() {
            continue;
        }
        let working = match scope.resolve_current_path(path) {
            Ok(path) => path,
            Err(_) => continue,
        };
        let mut guard = match super::WriteGuard::for_existing_file(scope, &working) {
            Ok(guard) => guard,
            Err(_) => continue,
        };
        if guard
            .read_existing()?
            .is_some_and(|current| current == bytes)
        {
            return Ok(Some(path.to_string()));
        }
    }
    Ok(None)
}

fn repository_relative(root: &Path, path: &Path) -> Result<String, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve Git project file: {error}"))?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| "project file is outside the Git repository".to_string())?;
    let value = relative.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        Err("project file path is empty".into())
    } else {
        Ok(value)
    }
}
