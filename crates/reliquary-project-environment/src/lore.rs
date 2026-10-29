use super::lore_repository::{
    LORE_DIR, LORE_IGNORE, create_repository, ensure_reserved_ignored, find_lore_root,
    project_relative_path, restore_ignore, validate_project_dir,
};
pub(crate) use super::lore_runtime::{block_on_lore, globals};
use lore::file::LoreFileStageArgs;
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreString};
use lore::revision::{LoreRevisionCommitArgs, LoreRevisionInfoArgs};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LoreRevisionState {
    pub(crate) repository_id: String,
    pub(crate) revision: String,
}

#[derive(Debug)]
pub(crate) struct PreparedLoreRepository {
    pub(crate) root: PathBuf,
    created_repository: bool,
    ignore_before: Option<Vec<u8>>,
}

impl PreparedLoreRepository {
    pub(crate) fn project_path(&self, project_dir: &Path) -> Result<String, String> {
        project_relative_path(&self.root, project_dir)
    }

    pub(crate) fn rollback(&self) {
        if self.created_repository {
            let _ = fs::remove_dir_all(self.root.join(LORE_DIR));
        }
        let ignore = self.root.join(LORE_IGNORE);
        match &self.ignore_before {
            Some(bytes) => {
                let _ = fs::write(ignore, bytes);
            }
            None => {
                let _ = fs::remove_file(ignore);
            }
        }
    }
}

pub(crate) fn prepare_managed_lore(project_dir: &Path) -> Result<PreparedLoreRepository, String> {
    validate_project_dir(project_dir)?;
    let root = find_lore_root(project_dir).unwrap_or_else(|| project_dir.to_path_buf());
    let created_repository = !root.join(LORE_DIR).is_dir() && !root.join(".urc").is_dir();
    if created_repository && let Err(error) = create_repository(&root, project_dir) {
        let _ = fs::remove_dir_all(root.join(LORE_DIR));
        return Err(error);
    }
    let ignore_path = root.join(LORE_IGNORE);
    let ignore_before = match fs::read(&ignore_path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            if created_repository {
                let _ = fs::remove_dir_all(root.join(LORE_DIR));
            }
            return Err(format!("failed to read existing Lore ignore file: {error}"));
        }
    };
    if let Err(error) = ensure_reserved_ignored(&root, project_dir) {
        restore_ignore(&root, ignore_before.as_deref());
        if created_repository {
            let _ = fs::remove_dir_all(root.join(LORE_DIR));
        }
        return Err(error);
    }
    Ok(PreparedLoreRepository {
        root,
        created_repository,
        ignore_before,
    })
}

pub(crate) fn require_lore_repository(
    project_dir: &Path,
) -> Result<PreparedLoreRepository, String> {
    validate_project_dir(project_dir)?;
    let root = find_lore_root(project_dir).ok_or_else(|| {
        format!(
            "managed Lore repository is missing for project: {}",
            project_dir.display()
        )
    })?;
    Ok(PreparedLoreRepository {
        root,
        created_repository: false,
        ignore_before: None,
    })
}

pub(crate) fn maintain_reserved_ignore(
    repository: &PreparedLoreRepository,
    project_dir: &Path,
) -> Result<(), String> {
    ensure_reserved_ignored(&repository.root, project_dir)
}

pub(crate) fn initial_checkpoint(
    repository: &PreparedLoreRepository,
    project_dir: &Path,
) -> Result<LoreRevisionState, String> {
    if repository.created_repository {
        let stage_path = project_relative_path(&repository.root, project_dir)?;
        let staged = Arc::new(Mutex::new(0usize));
        let staged_callback = staged.clone();
        let callback: LoreEventCallback = Some(Box::new(move |event: &LoreEvent| {
            if matches!(event, LoreEvent::FileStageFile(_)) {
                *staged_callback.lock().expect("Lore stage counter poisoned") += 1;
            }
        }));
        let status = block_on_lore(lore::file::stage(
            globals(&repository.root),
            LoreFileStageArgs {
                paths: LoreArray::from_vec(vec![LoreString::from(stage_path)]),
                case_change: 0,
                scan: 1,
            },
            callback,
        ));
        if status != 0 {
            return Err(format!("Lore initial stage failed with status {status}"));
        }
        let staged_count = *staged.lock().map_err(|_| "Lore stage counter poisoned")?;
        let mut commit_globals = globals(&repository.root);
        if staged_count == 0 {
            commit_globals.force = 1;
        }
        let status = block_on_lore(lore::revision::commit(
            commit_globals,
            LoreRevisionCommitArgs {
                message: "Initialize Warlock project".into(),
                ..Default::default()
            },
            None,
        ));
        if status != 0 {
            return Err(format!("Lore initial commit failed with status {status}"));
        }
    }
    current_revision(repository)
}

pub(crate) fn current_revision(
    repository: &PreparedLoreRepository,
) -> Result<LoreRevisionState, String> {
    let state = Arc::new(Mutex::new(None::<LoreRevisionState>));
    let captured = state.clone();
    let callback: LoreEventCallback = Some(Box::new(move |event: &LoreEvent| {
        if let LoreEvent::RevisionInfo(data) = event {
            *captured.lock().expect("Lore revision state poisoned") = Some(LoreRevisionState {
                repository_id: data.repository.to_string(),
                revision: data.revision.to_string(),
            });
        }
    }));
    let status = block_on_lore(lore::revision::info(
        globals(&repository.root),
        LoreRevisionInfoArgs {
            revision: LoreString::default(),
            delta: 0,
            metadata: 0,
        },
        callback,
    ));
    if status != 0 {
        return Err(format!(
            "Lore current revision lookup failed with status {status}"
        ));
    }
    state
        .lock()
        .map_err(|_| "Lore revision state poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Lore current revision lookup returned no revision".to_string())
}
