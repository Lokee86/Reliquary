use super::lore::PreparedLoreRepository;
use super::lore_file::{read_lore_path, repository_relative, sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn find_tracked_match(
    repository: &PreparedLoreRepository,
    project_dir: &Path,
    byte_length: u64,
    content_hash: [u8; 32],
    revision: &str,
) -> Result<Option<String>, String> {
    let mut files = Vec::new();
    // Search ordinary project content, but keep the reserved project data tree out of
    // the general scan. Tracked uploads are admitted explicitly below; generated
    // artifacts are never attachment-dedupe candidates.
    collect_candidates(project_dir, &mut files, true)?;
    let uploads = project_dir.join("warlock/uploads");
    if uploads.is_dir() {
        collect_candidates(&uploads, &mut files, false)?;
    }
    files.sort();
    for file in files {
        let metadata = match fs::metadata(&file) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() != byte_length {
            continue;
        }
        let local = match fs::read(&file) {
            Ok(bytes) if sha256(&bytes) == content_hash => bytes,
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
    project_dir: &Path,
    filename: &str,
    content_hash: [u8; 32],
) -> Result<PathBuf, String> {
    let warlock = project_dir.join("warlock");
    let uploads = warlock.join("uploads");
    let generated = warlock.join("generated");
    fs::create_dir_all(&uploads)
        .map_err(|error| format!("failed to create project uploads directory: {error}"))?;
    fs::create_dir_all(&generated)
        .map_err(|error| format!("failed to create project generated directory: {error}"))?;
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
        let candidate = uploads.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
        if candidate.is_file()
            && fs::read(&candidate).is_ok_and(|bytes| sha256(&bytes) == content_hash)
        {
            return Ok(candidate);
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
    root: &Path,
    files: &mut Vec<PathBuf>,
    skip_legacy_data_root: bool,
) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| format!("failed to scan project: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to scan project: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            if !(skip_legacy_data_root && name == "warlock") && !excluded_dir(&name) {
                collect_candidates(&path, files, false)?;
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
