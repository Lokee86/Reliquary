use super::PreparedProjectAttachment;
use super::lore::{
    PreparedLoreRepository, block_on_lore, current_revision, globals, maintain_reserved_ignore,
    require_lore_repository,
};
use super::lore_file::{repository_relative, sha256};
use super::upload_paths::{find_tracked_match, mime_type, upload_destination};
use lore::file::LoreFileStageArgs;
use lore::interface::{LoreArray, LoreString};
use lore::revision::LoreRevisionCommitArgs;
use reliquary_archive::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
};
use std::fs;
use std::path::Path;

pub(crate) fn prepare_upload(
    project_dir: &Path,
    source: &Path,
) -> Result<PreparedProjectAttachment, String> {
    prepare_upload_inner(project_dir, source)
}

pub(crate) fn prepare_manual_upload(
    project_dir: &Path,
    source: &Path,
) -> Result<PreparedProjectAttachment, String> {
    // Manual repository management excludes routine Warlock checkpoints, but attachment
    // ingestion is an explicit exception so external files can still acquire exact provenance.
    prepare_upload_inner(project_dir, source)
}

fn prepare_upload_inner(
    project_dir: &Path,
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
    let byte_length = bytes.len() as u64;
    let content_hash = sha256(&bytes);
    let repository = require_lore_repository(project_dir)?;
    maintain_reserved_ignore(&repository, project_dir)?;
    let current = current_revision(&repository)?;

    let (path, revision) = match find_tracked_match(
        &repository,
        project_dir,
        byte_length,
        content_hash,
        &current.revision,
    )? {
        Some(path) => (path, current),
        None => {
            let destination = upload_destination(project_dir, &filename, content_hash)?;
            if destination != source {
                fs::write(&destination, &bytes)
                    .map_err(|error| format!("failed to materialize project upload: {error}"))?;
            }
            let path = repository_relative(&repository.root, &destination)?;
            commit_file(&repository, &path, &filename)?;
            (path, current_revision(&repository)?)
        }
    };

    Ok(PreparedProjectAttachment {
        filename: filename.clone(),
        mime_type: mime_type(&filename),
        byte_length,
        reference: ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Lore,
                    repository_id: revision.repository_id,
                    project_path: repository.project_path(project_dir)?,
                },
                revision: revision.revision,
            },
            path,
            content_hash: Some(content_hash),
        },
    })
}

fn commit_file(
    repository: &PreparedLoreRepository,
    path: &str,
    filename: &str,
) -> Result<(), String> {
    let stage_path = repository.root.join(path).to_string_lossy().to_string();
    let status = block_on_lore(lore::file::stage(
        globals(&repository.root),
        LoreFileStageArgs {
            paths: LoreArray::from_vec(vec![LoreString::from(stage_path)]),
            case_change: 0,
            scan: 0,
        },
        None,
    ));
    if status != 0 {
        return Err(format!("Lore attachment stage failed with status {status}"));
    }
    let status = block_on_lore(lore::revision::commit(
        globals(&repository.root),
        LoreRevisionCommitArgs {
            message: format!("Add Warlock context file {filename}").into(),
            ..Default::default()
        },
        None,
    ));
    if status == 0 {
        Ok(())
    } else {
        Err(format!(
            "Lore attachment commit failed with status {status}"
        ))
    }
}
