use crate::{Container, ContainerIdentity, Cva, CvaError, FileKind, Phylactery, PhylacteryError};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TailRepairOutcome {
    NoIncompleteTail,
    Repaired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityResolutionMaintenance {
    NothingToRepair,
    RetiredEntityReferences { revisions: usize },
}

struct StagedPrefix {
    path: PathBuf,
    guard: Option<crate::container::PhysicalReadSource>,
}

impl StagedPrefix {
    fn new(path: PathBuf) -> Self {
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

fn staged_path(source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("owner");
    source.with_file_name(format!(".{name}.repair-{}.stage", uuid::Uuid::new_v4()))
}

impl Cva {
    /// Reports whether current resolution rows need explicit retired-Entity canonicalization.
    pub fn entity_resolution_maintenance(&self) -> Result<EntityResolutionMaintenance, CvaError> {
        let revisions = self.pending_retired_entity_reference_repairs_count()?;
        Ok(if revisions == 0 {
            EntityResolutionMaintenance::NothingToRepair
        } else {
            EntityResolutionMaintenance::RetiredEntityReferences { revisions }
        })
    }

    /// Validates and repairs an incomplete REL tail under one exclusive source lease.
    /// The accepted prefix is replayed from a staged copy before the source is truncated.
    pub fn repair_incomplete_tail(path: impl AsRef<Path>) -> Result<TailRepairOutcome, CvaError> {
        let path = path.as_ref();
        let mut source = Container::open_for_repair(path)?;
        if source.owner_uuid().is_none() {
            return Err(crate::ContainerError::InvalidIdentity.into());
        }
        if source.identity().map(|identity| identity.file_kind) != Some(FileKind::Reliquary) {
            return Err(CvaError::InvalidContainerIdentity(
                "file is not a Reliquary",
            ));
        }
        let Some(plan) = source.plan_tail_repair()? else {
            let _validated = Self::open_from_container(source)?;
            return Ok(TailRepairOutcome::NoIncompleteTail);
        };

        let mut staged = StagedPrefix::new(staged_path(path));
        let staged_container = source.copy_accepted_prefix(&plan, &staged.path)?;
        staged.guard = Some(staged_container.read_source()?);
        let validated = Cva::open_from_container(staged_container)?;
        if validated.open_import_epoch().is_some() {
            return Err(crate::ContainerError::WriteAdmissionClosed.into());
        }
        drop(validated);

        source.repair_incomplete_tail(plan)?;
        let repaired = Cva::open_from_container(source)?;
        drop(repaired);
        Ok(TailRepairOutcome::Repaired)
    }

    /// Explicitly reclaims stale Archive conversation-compaction chunks discovered during replay.
    /// Ordinary open keeps the current projection and never performs this maintenance.
    pub fn maintain_conversation_compactions(
        &mut self,
    ) -> Result<crate::ConversationCompactionMaintenance, CvaError> {
        Ok(self.conversation_compactions.recover(&mut self.container)?)
    }

    /// Explicitly appends canonical resolution revisions for references to retired Entities.
    /// Opening a REL never invokes this repair automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, CvaError> {
        Ok(self.apply_retired_entity_reference_repairs()?)
    }
}

impl Phylactery {
    /// Reports whether current resolution rows need explicit retired-Entity canonicalization.
    pub fn entity_resolution_maintenance(
        &self,
    ) -> Result<EntityResolutionMaintenance, PhylacteryError> {
        let revisions = self.pending_retired_entity_reference_repairs_count()?;
        Ok(if revisions == 0 {
            EntityResolutionMaintenance::NothingToRepair
        } else {
            EntityResolutionMaintenance::RetiredEntityReferences { revisions }
        })
    }

    /// Validates and repairs an incomplete PHY tail under one exclusive source lease.
    /// The accepted prefix is replayed from a staged copy before the source is truncated.
    pub fn repair_incomplete_tail(
        path: impl AsRef<Path>,
    ) -> Result<TailRepairOutcome, PhylacteryError> {
        let path = path.as_ref();
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
            return Err(PhylacteryError::InvalidContainerIdentity(
                "file is not a Phylactery",
            ));
        }
        let Some(plan) = source.plan_tail_repair()? else {
            let _validated = Self::open_from_container(source)?;
            return Ok(TailRepairOutcome::NoIncompleteTail);
        };

        let mut staged = StagedPrefix::new(staged_path(path));
        let staged_container = source.copy_accepted_prefix(&plan, &staged.path)?;
        staged.guard = Some(staged_container.read_source()?);
        let validated = Phylactery::open_from_container(staged_container)?;
        drop(validated);

        source.repair_incomplete_tail(plan)?;
        let repaired = Phylactery::open_from_container(source)?;
        drop(repaired);
        Ok(TailRepairOutcome::Repaired)
    }

    /// Explicitly appends canonical resolution revisions for references to retired Entities.
    /// Opening a PHY never invokes this repair automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, PhylacteryError> {
        Ok(self.apply_retired_entity_reference_repairs()?)
    }
}
