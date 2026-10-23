//! Explicit accepted-prefix tail repair under the existing exclusive source lease.
use crate::owner_lifecycle::OwnerPhylacteryError;
use crate::{Container, ContainerIdentity, FileKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TailRepairOutcome {
    NoIncompleteTail,
    Repaired,
}

pub(crate) struct StagedPrefix {
    pub(crate) path: PathBuf,
    pub(crate) guard: Option<crate::container::PhysicalReadSource>,
}

impl StagedPrefix {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, guard: None }
    }
}

impl Drop for StagedPrefix {
    fn drop(&mut self) {
        if self
            .guard
            .as_ref()
            .is_some_and(|guard| guard.validate_path_identity().is_ok())
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn staged_path(source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("owner");
    source.with_file_name(format!(".{name}.repair-{}.stage", uuid::Uuid::new_v4()))
}

pub(crate) fn repair_phylactery(path: &Path) -> Result<TailRepairOutcome, OwnerPhylacteryError> {
    let mut source = Container::open_for_repair(path)?;
    if source.owner_uuid().is_none() {
        return Err(crate::ContainerError::InvalidIdentity.into());
    }
    if source.identity()
        != Some(ContainerIdentity {
            file_kind: FileKind::Phylactery,
            scope: None,
        })
    {
        return Err(OwnerPhylacteryError::InvalidContainerIdentity(
            "file is not a Phylactery",
        ));
    }
    let Some(plan) = source.plan_tail_repair()? else {
        let _validated = super::phylactery_copy::reopen(source)?;
        return Ok(TailRepairOutcome::NoIncompleteTail);
    };

    let mut staged = StagedPrefix::new(staged_path(path));
    let staged_container = source.copy_accepted_prefix(&plan, &staged.path)?;
    staged.guard = Some(staged_container.read_source()?);
    let validated = super::phylactery_copy::reopen(staged_container)?;
    drop(validated);

    source.repair_incomplete_tail(plan)?;
    let repaired = super::phylactery_copy::reopen(source)?;
    drop(repaired);
    Ok(TailRepairOutcome::Repaired)
}

/// Validate and repair an incomplete REL tail through canonical History
/// reconstruction while retaining the existing exclusive source lease.
pub(crate) fn repair_reliquary(
    path: &Path,
) -> Result<TailRepairOutcome, crate::owner_lifecycle::OwnerError> {
    let mut source = Container::open_for_repair(path)?;
    if source.owner_uuid().is_none() {
        return Err(crate::ContainerError::InvalidIdentity.into());
    }
    if source.identity().map(|identity| identity.file_kind) != Some(FileKind::Reliquary) {
        return Err(
            crate::owner_lifecycle::OwnerError::InvalidContainerIdentity("file is not a Reliquary"),
        );
    }
    let Some(plan) = source.plan_tail_repair()? else {
        let _validated = super::reliquary_prune::reopen_reliquary(source)?;
        return Ok(TailRepairOutcome::NoIncompleteTail);
    };

    let mut staged = StagedPrefix::new(staged_path(path));
    let staged_container = source.copy_accepted_prefix(&plan, &staged.path)?;
    staged.guard = Some(staged_container.read_source()?);
    let (validated, _) = super::reliquary_prune::reopen_reliquary(staged_container)?;
    if validated.import_epochs.open().is_some() {
        return Err(crate::ContainerError::WriteAdmissionClosed.into());
    }
    drop(validated);

    source.repair_incomplete_tail(plan)?;
    let _repaired = super::reliquary_prune::reopen_reliquary(source)?;
    Ok(TailRepairOutcome::Repaired)
}
