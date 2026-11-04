//! Read and checkpoint history through Lore's repository API.
//!
//! Lore owns its repository identity, revisions, and historical file bytes.
//! Archive references are correlation metadata and never replace those checks.

use lore::file::{LoreFileStageArgs, LoreFileWriteArgs};
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreGlobalArgs, LoreString};
use lore::revision::{LoreRevisionCommitArgs, LoreRevisionInfoArgs};
use reliquary_archive::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
    validate_project_file_reference,
};
use sha2::Digest;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const LORE_DIR: &str = ".lore";
const LEGACY_LORE_DIR: &str = ".urc";
const LORE_IGNORE: &str = ".loreignore";
const RESERVED_DIR: &str = ".reliquary/";

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoreRevisionState {
    repository_id: String,
    revision: String,
}

/// Read exact historical bytes after checking the Archive reference against
/// the open Lore repository's own identity.
pub fn read_lore_project_file(
    project_dir: &Path,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    validate_project_file_reference(reference).map_err(|error| error.to_string())?;
    if reference.revision.repository.kind != ProjectRepositoryKind::Lore {
        return Err("Project file reference is not backed by Lore".into());
    }
    let root = find_lore_root(project_dir)?;
    let current = current_revision(&root)?;
    if current.repository_id != reference.revision.repository.repository_id {
        return Err("Project file repository identity does not match the open Lore project".into());
    }
    let output = temporary_path(&root)?;
    let historical = root.join(&reference.path);
    let status = block_on_lore(lore::file::write(
        globals(&root),
        LoreFileWriteArgs {
            address: LoreString::default(),
            path: historical.to_string_lossy().as_ref().into(),
            revision: reference.revision.revision.as_str().into(),
            output: output.to_string_lossy().as_ref().into(),
        },
        None,
    ));
    if status != 0 {
        let _ = fs::remove_file(&output);
        return Err(format!(
            "Lore historical file read failed with status {status}"
        ));
    }
    let result = fs::read(&output).map_err(|error| format!("failed to read Lore output: {error}"));
    let _ = fs::remove_file(output);
    let bytes = result?;
    if reference
        .content_hash
        .is_some_and(|hash| <[u8; 32]>::from(sha2::Sha256::digest(&bytes)) != hash)
    {
        return Err("Historical project file content hash mismatch".into());
    }
    Ok(bytes)
}

/// Explicitly stage the project subtree and commit only when Lore reports
/// staged files. The returned revision is read back from Lore after commit.
pub fn checkpoint_lore_project(project_dir: &Path) -> Result<ProjectRevisionRef, String> {
    if !project_dir.is_dir() {
        return Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ));
    }
    let root = find_lore_root(project_dir)?;
    ensure_reserved_ignored(&root)?;
    let project_path = project_dir
        .strip_prefix(&root)
        .map_err(|_| "project folder is outside the Lore repository root".to_string())?;
    // Lore resolves user paths relative to the process working directory,
    // not the repository root. Pass an absolute path for this project subtree.
    let stage_path = project_dir.to_string_lossy().replace('\\', "/");
    let staged = Arc::new(Mutex::new(0usize));
    let staged_callback = staged.clone();
    let callback: LoreEventCallback = Some(Box::new(move |event: &LoreEvent| {
        if let LoreEvent::FileStageEnd(data) = event {
            *staged_callback.lock().expect("Lore stage counter poisoned") =
                data.count.total_count as usize;
        }
    }));
    let status = block_on_lore(lore::file::stage(
        globals(&root),
        LoreFileStageArgs {
            paths: LoreArray::from_vec(vec![stage_path.into()]),
            case_change: 0,
            scan: 1,
        },
        callback,
    ));
    if status != 0 {
        return Err(format!("Lore project stage failed with status {status}"));
    }
    if *staged.lock().map_err(|_| "Lore stage counter poisoned")? > 0 {
        let status = block_on_lore(lore::revision::commit(
            globals(&root),
            LoreRevisionCommitArgs {
                message: "Checkpoint project".into(),
                ..Default::default()
            },
            None,
        ));
        if status != 0 {
            return Err(format!(
                "Lore project checkpoint failed with status {status}"
            ));
        }
    }
    let current = current_revision(&root)?;
    let project_path = if project_path.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        project_path.to_string_lossy().replace('\\', "/")
    };
    Ok(ProjectRevisionRef {
        repository: ProjectRepositoryRef {
            kind: ProjectRepositoryKind::Lore,
            repository_id: current.repository_id,
            project_path,
        },
        revision: current.revision,
    })
}

fn find_lore_root(project_dir: &Path) -> Result<PathBuf, String> {
    if !project_dir.is_dir() {
        return Err(format!(
            "project folder does not exist or is not a directory: {}",
            project_dir.display()
        ));
    }
    project_dir
        .ancestors()
        .find(|path| path.join(LORE_DIR).is_dir() || path.join(LEGACY_LORE_DIR).is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| "managed Lore repository is missing for project".into())
}

fn current_revision(root: &Path) -> Result<LoreRevisionState, String> {
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
        globals(root),
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
        .ok_or_else(|| "Lore current revision lookup returned no revision".into())
}

fn ensure_reserved_ignored(root: &Path) -> Result<(), String> {
    let path = root.join(LORE_IGNORE);
    let mut contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read existing Lore ignore file: {error}")),
    };
    if contents.lines().any(|line| line.trim() == RESERVED_DIR) {
        return Ok(());
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(RESERVED_DIR);
    contents.push('\n');
    fs::write(path, contents)
        .map_err(|error| format!("failed to configure Lore ignore file: {error}"))
}

fn temporary_path(root: &Path) -> Result<PathBuf, String> {
    let directory = if root.join(LORE_DIR).is_dir() {
        root.join(LORE_DIR)
    } else {
        root.join(LEGACY_LORE_DIR)
    };
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    Ok(directory.join(format!(
        ".project-file-read-{}-{suffix}",
        std::process::id()
    )))
}

fn globals(root: &Path) -> LoreGlobalArgs {
    LoreGlobalArgs {
        repository_path: root.to_string_lossy().as_ref().into(),
        offline: 1,
        local: 1,
        ..Default::default()
    }
}

fn block_on_lore<F>(future: F) -> F::Output
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::scope(|scope| {
            scope
                .spawn(move || lore::runtime().block_on(future))
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    } else {
        lore::runtime().block_on(future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lore::interface::LoreSharedStoreMode;
    use lore::repository::LoreRepositoryCreateArgs;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".test-tmp")
                .join(format!(
                    "lore-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lore_checkpoint_is_idempotent_and_reads_archived_revision() {
        let temp = TestDirectory::new();
        let root = temp.0.join("repository");
        let project = root.join("client");
        fs::create_dir_all(project.join("docs")).unwrap();
        fs::write(project.join("docs/record.bin"), [3, 4, 5, 255]).unwrap();
        let status = block_on_lore(lore::repository::create(
            globals(&root),
            LoreRepositoryCreateArgs {
                repository_url: "project-environment-test".into(),
                id: LoreString::default(),
                description: "Project environment test repository".into(),
                use_shared_store: LoreSharedStoreMode::Disabled,
                shared_store_path: LoreString::default(),
            },
            None,
        ));
        assert_eq!(status, 0);

        let initial = checkpoint_lore_project(&project).unwrap();
        let repeated = checkpoint_lore_project(&project).unwrap();
        assert_eq!(initial, repeated);

        fs::write(project.join("docs/record.bin"), b"changed later").unwrap();
        checkpoint_lore_project(&project).unwrap();
        let reference = ProjectFileRef {
            revision: initial,
            path: "client/docs/record.bin".into(),
            content_hash: Some(sha2::Sha256::digest([3, 4, 5, 255]).into()),
        };
        assert_eq!(
            read_lore_project_file(&project, &reference).unwrap(),
            [3, 4, 5, 255]
        );
    }
}
