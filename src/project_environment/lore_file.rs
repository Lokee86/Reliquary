use super::lore::{PreparedLoreRepository, block_on_lore, globals};
use super::scope::ValidatedProjectScope;
use crate::ProjectFileRef;
use lore::file::LoreFileWriteArgs;
use lore::interface::LoreString;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub(crate) fn read_lore_project_file(
    scope: &ValidatedProjectScope,
    reference: &ProjectFileRef,
) -> Result<Vec<u8>, String> {
    scope.authorize_historical(reference)?;
    let repository = scope.lore_repository()?;
    let bytes = read_lore_path(&repository, &reference.path, &reference.revision.revision)?;
    if let Some(expected) = reference.content_hash
        && sha256(&bytes) != expected
    {
        return Err("Historical project file content hash mismatch".into());
    }
    Ok(bytes)
}

pub(crate) fn read_lore_path(
    repository: &PreparedLoreRepository,
    path: &str,
    revision: &str,
) -> Result<Vec<u8>, String> {
    let output = repository
        .root
        .join(if repository.root.join(".lore").is_dir() {
            ".lore"
        } else {
            ".urc"
        })
        .join(format!("warlock-read-{}", Uuid::new_v4()));
    let historical_path = repository.root.join(path).to_string_lossy().to_string();
    let status = block_on_lore(lore::file::write(
        globals(&repository.root),
        LoreFileWriteArgs {
            address: LoreString::default(),
            path: historical_path.into(),
            revision: revision.into(),
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
    result
}

pub(crate) fn repository_relative(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "project file is outside the Lore repository".to_string())?;
    let value = relative.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        Err("project file path is empty".into())
    } else {
        Ok(value)
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
