use super::lore::PreparedLoreRepository;
use super::lore_file::{read_lore_path, repository_relative, sha256};
use super::scope::ValidatedProjectScope;
use super::write_guard::WriteGuard;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn find_tracked_match(
    repository: &PreparedLoreRepository,
    scope: &ValidatedProjectScope,
    byte_length: u64,
    content_hash: [u8; 32],
    revision: &str,
) -> Result<Option<String>, String> {
    let project_dir = scope.project_dir();
    let mut files = Vec::new();
    // Search ordinary project content, but keep the reserved project data tree out of
    // the general scan. Tracked uploads are admitted explicitly below; generated
    // artifacts are never attachment-dedupe candidates.
    collect_candidates(scope, project_dir, &mut files, true)?;
    let uploads = project_dir.join("warlock/uploads");
    if uploads.is_dir() {
        collect_candidates(scope, &uploads, &mut files, false)?;
    }
    files.sort();
    for file in files {
        let logical = repository_relative(&repository.root, &file)?;
        if scope.resolve_current_path(&logical).is_err() {
            continue;
        }
        let metadata = match fs::metadata(&file) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() != byte_length {
            continue;
        }
        let mut guard = match WriteGuard::for_existing_file(scope, &file) {
            Ok(guard) => guard,
            Err(_) => continue,
        };
        let local = match guard.read_existing()? {
            Some(bytes) if sha256(&bytes) == content_hash => bytes,
            _ => continue,
        };
        let path = repository_relative(&repository.root, &file)?;
        let historical = match read_lore_path(repository, &path, revision) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if historical.len() == local.len() && sha256(&historical) == content_hash {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub(crate) fn upload_destination(
    scope: &ValidatedProjectScope,
    filename: &str,
    content_hash: [u8; 32],
) -> Result<WriteGuard, String> {
    scope.revalidate()?;
    let project_dir = scope.project_dir();
    let prefix = if scope.project_path() == "." {
        String::new()
    } else {
        format!("{}/", scope.project_path())
    };
    scope.resolve_current_path(&format!("{prefix}warlock/uploads/{filename}"))?;
    scope.resolve_current_path(&format!("{prefix}warlock/generated/check"))?;
    let warlock = project_dir.join("warlock");
    let uploads = warlock.join("uploads");
    let generated = warlock.join("generated");
    let _generated = WriteGuard::for_path(scope, &generated.join("reserved-placeholder"))?;
    let original = Path::new(filename);
    let stem = original
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = original.extension().and_then(|value| value.to_str());
    for index in 1.. {
        let name = if index == 1 {
            filename.to_string()
        } else if let Some(extension) = extension {
            format!("{stem}-{index}.{extension}")
        } else {
            format!("{stem}-{index}")
        };
        let mut candidate = WriteGuard::for_path(scope, &uploads.join(&name))?;
        match candidate.read_existing()? {
            None => return Ok(candidate),
            Some(bytes) if sha256(&bytes) == content_hash => return Ok(candidate),
            Some(_) => {}
        }
    }
    unreachable!()
}

pub(crate) fn mime_type(filename: &str) -> Option<String> {
    let extension = Path::new(filename)
        .extension()?
        .to_str()?
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "csv" => "text/csv",
        _ => return None,
    };
    Some(mime.into())
}

fn collect_candidates(
    scope: &ValidatedProjectScope,
    root: &Path,
    files: &mut Vec<PathBuf>,
    skip_legacy_data_root: bool,
) -> Result<(), String> {
    let _directory = WriteGuard::for_directory(scope, root)?;
    for entry in fs::read_dir(root).map_err(|error| format!("failed to scan project: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to scan project: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_symlink()
        {
            continue;
        }
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            if !(skip_legacy_data_root && name == "warlock") && !excluded_dir(&name) {
                collect_candidates(scope, &path, files, false)?;
            }
        } else if name != ".loreignore" {
            files.push(path);
        }
    }
    Ok(())
}

fn excluded_dir(name: &str) -> bool {
    matches!(
        name,
        ".warlock"
            | ".lore"
            | ".urc"
            | ".git"
            | ".cache"
            | "cache"
            | "node_modules"
            | "target"
            | "build"
            | "dist"
            | "vendor"
            | "tmp"
            | "temp"
    )
}

/// Refuse redirected checkpoint inputs before asking Lore to scan the selected tree.
pub(crate) fn validate_checkpoint_tree(
    scope: &ValidatedProjectScope,
) -> Result<Vec<WriteGuard>, String> {
    fn visit(
        scope: &ValidatedProjectScope,
        dir: &Path,
        guards: &mut Vec<WriteGuard>,
    ) -> Result<(), String> {
        guards.push(WriteGuard::for_directory(scope, dir)?);
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if matches!(name.as_str(), ".warlock" | ".git" | ".lore" | ".urc") {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            let logical = repository_relative(scope.repository_root(), &entry.path())?;
            scope.resolve_current_path(&logical)?;
            if metadata.file_type().is_symlink() {
                return Err("checkpoint contains a redirected project path".into());
            }
            if metadata.is_dir() {
                visit(scope, &entry.path(), guards)?;
            } else {
                guards.push(WriteGuard::for_existing_file(scope, &entry.path())?);
            }
        }
        Ok(())
    }
    let mut guards = Vec::new();
    visit(scope, scope.project_dir(), &mut guards)?;
    Ok(guards)
}

/// A new upload never overwrites an existing file or follows a final symlink.
pub(crate) fn materialize(
    scope: &ValidatedProjectScope,
    destination: &mut WriteGuard,
    bytes: &[u8],
) -> Result<(), String> {
    scope.revalidate()?;
    destination.materialize(bytes)?;
    scope.revalidate()?;
    Ok(())
}
