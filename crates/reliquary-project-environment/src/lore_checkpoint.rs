use super::lore::{
    block_on_lore, current_revision, globals, maintain_reserved_ignore, require_lore_repository,
};
use lore::file::LoreFileStageArgs;
use lore::interface::{LoreArray, LoreEvent, LoreEventCallback, LoreString};
use lore::revision::LoreRevisionCommitArgs;
use reliquary_archive::{ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub(crate) fn checkpoint_project(project_dir: &Path) -> Result<ProjectRevisionRef, String> {
    let repository = require_lore_repository(project_dir)?;
    maintain_reserved_ignore(&repository, project_dir)?;
    let stage_path = repository.project_path(project_dir)?;
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
        return Err(format!("Lore project stage failed with status {status}"));
    }
    let staged_count = *staged.lock().map_err(|_| "Lore stage counter poisoned")?;
    if staged_count > 0 {
        let status = block_on_lore(lore::revision::commit(
            globals(&repository.root),
            LoreRevisionCommitArgs {
                message: "Checkpoint Warlock project".into(),
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
    let current = current_revision(&repository)?;
    Ok(ProjectRevisionRef {
        repository: ProjectRepositoryRef {
            kind: ProjectRepositoryKind::Lore,
            repository_id: current.repository_id,
            project_path: repository.project_path(project_dir)?,
        },
        revision: current.revision,
    })
}
