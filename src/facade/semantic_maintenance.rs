use crate::maintenance::entity_resolution::EntityResolutionMaintenance;
use crate::maintenance::tail_repair::TailRepairOutcome;
use crate::{Cva, CvaError, Phylactery, PhylacteryError};
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
        crate::maintenance::tail_repair::repair_reliquary(path.as_ref())
    }

    /// Explicitly reclaims stale Archive conversation-compaction chunks discovered during replay.
    /// Ordinary open keeps the current projection and never performs this maintenance.
    pub fn maintain_conversation_compactions(
        &mut self,
    ) -> Result<crate::ConversationCompactionMaintenance, CvaError> {
        Ok(
            crate::maintenance::compactions::maintain_conversation_compactions(
                &mut self.owner.container,
                &mut self.owner.conversation_compactions,
            )?,
        )
    }

    /// Explicitly appends canonical resolution revisions for references to retired Entities.
    /// Opening a REL never invokes this repair automatically.
    pub fn repair_retired_entity_references(&mut self) -> Result<usize, CvaError> {
        Ok(crate::maintenance::entity_resolution::repair(
            &mut self.owner.container,
            &mut self.owner.entity_resolutions,
            &self.owner.memories,
            &self.owner.entities,
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
            &mut self.owner.container,
            &mut self.owner.entity_resolutions,
            &self.owner.memories,
            &self.owner.entities,
        )?)
    }
}
