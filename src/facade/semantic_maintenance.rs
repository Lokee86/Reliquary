use crate::maintenance::entity_resolution::EntityResolutionMaintenance;
use crate::maintenance::tail_repair::{StagedPrefix, TailRepairOutcome, staged_path};
use crate::{Container, Cva, CvaError, FileKind, Phylactery, PhylacteryError};
use std::path::Path;

impl Cva {
    /// Reports whether current resolution rows need explicit retired-Entity canonicalization.
    pub fn entity_resolution_maintenance(&self) -> Result<EntityResolutionMaintenance, CvaError> {
        Ok(crate::maintenance::entity_resolution::inspect(
            &self.entity_resolutions,
            &self.memories,
            &self.entities,
        )?)
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
        Ok(
            crate::maintenance::compactions::maintain_conversation_compactions(
                &mut self.container,
                &mut self.conversation_compactions,
            )?,
        )
    }

    /// Explicitly appends canonical resolution revisions for references to retired Entities.
    /// Opening a REL never invokes this repair automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, CvaError> {
        Ok(crate::maintenance::entity_resolution::repair(
            &mut self.container,
            &mut self.entity_resolutions,
            &self.memories,
            &self.entities,
        )?)
    }
}

impl Phylactery {
    /// Reports whether current resolution rows need explicit retired-Entity canonicalization.
    pub fn entity_resolution_maintenance(
        &self,
    ) -> Result<EntityResolutionMaintenance, PhylacteryError> {
        Ok(crate::maintenance::entity_resolution::inspect(
            &self.entity_resolutions,
            &self.memories,
            &self.entities,
        )?)
    }

    /// Validates and repairs an incomplete PHY tail under one exclusive source lease.
    /// The accepted prefix is replayed from a staged copy before the source is truncated.
    pub fn repair_incomplete_tail(
        path: impl AsRef<Path>,
    ) -> Result<TailRepairOutcome, PhylacteryError> {
        crate::maintenance::tail_repair::repair_phylactery(path.as_ref())
    }

    /// Explicitly appends canonical resolution revisions for references to retired Entities.
    /// Opening a PHY never invokes this repair automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, PhylacteryError> {
        Ok(crate::maintenance::entity_resolution::repair(
            &mut self.container,
            &mut self.entity_resolutions,
            &self.memories,
            &self.entities,
        )?)
    }
}
