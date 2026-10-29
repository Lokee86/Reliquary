use super::lore_runtime::{block_on_lore, globals};
use lore::interface::{LoreSharedStoreMode, LoreString};
use lore::repository::LoreRepositoryCreateArgs;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const LORE_DIR: &str = ".lore";
const LEGACY_LORE_DIR: &str = ".urc";
pub(crate) const LORE_IGNORE: &str = ".loreignore";
const LEGACY_RESERVED_DIR: &str = ".warlock";

pub(crate) fn validate_project_dir(project_dir: &Path) -> Result<(), String> {
    if project_dir.is_dir() {
        Ok(())
    } else {
        Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ))
    }
}

pub(crate) fn create_repository(root: &Path, project_dir: &Path) -> Result<(), String> {
    let status = block_on_lore(lore::repository::create(
        globals(root),
        LoreRepositoryCreateArgs {
            repository_url: repository_name(project_dir).into(),
            id: LoreString::default(),
            description: "Warlock managed project".into(),
            use_shared_store: LoreSharedStoreMode::Disabled,
            shared_store_path: LoreString::default(),
        },
        None,
    ));
    if status == 0 {
        Ok(())
    } else {
        Err(format!(
            "Lore repository creation failed with status {status}"
        ))
    }
}

pub(crate) fn restore_ignore(root: &Path, previous: Option<&[u8]>) {
    let path = root.join(LORE_IGNORE);
    match previous {
        Some(bytes) => {
            let _ = fs::write(path, bytes);
        }
        None => {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn ensure_reserved_ignored(root: &Path, project_dir: &Path) -> Result<(), String> {
    let ignore_path = root.join(LORE_IGNORE);
    let mut current = match fs::read_to_string(&ignore_path) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read existing Lore ignore file: {error}")),
    };
    let mut relative = project_dir
        .strip_prefix(root)
        .map_err(|_| "project folder is outside the Lore repository root".to_string())?
        .join(LEGACY_RESERVED_DIR)
        .to_string_lossy()
        .replace('\\', "/");
    if relative.is_empty() {
        relative = LEGACY_RESERVED_DIR.to_string();
    }
    relative.push('/');
    if current.lines().any(|line| line.trim() == relative) {
        return Ok(());
    }
    if !current.is_empty() && !current.ends_with('\n') {
        current.push('\n');
    }
    current.push_str(&relative);
    current.push('\n');
    fs::write(&ignore_path, current)
        .map_err(|error| format!("failed to configure Lore ignore file: {error}"))
}

pub(crate) fn find_lore_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|candidate| has_lore_repository(candidate))
        .map(Path::to_path_buf)
}

pub(crate) fn project_relative_path(root: &Path, project_dir: &Path) -> Result<String, String> {
    let relative = project_dir
        .strip_prefix(root)
        .map_err(|_| "project folder is outside the Lore repository root".to_string())?;
    if relative.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}

fn has_lore_repository(path: &Path) -> bool {
    path.join(LORE_DIR).is_dir() || path.join(LEGACY_LORE_DIR).is_dir()
}

fn repository_name(project_dir: &Path) -> String {
    let raw = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("warlock-project");
    let mut name = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while name.starts_with('.') {
        name.remove(0);
    }
    if name.is_empty() {
        "warlock-project".to_string()
    } else {
        name
    }
}
