//! Authorized host context and compaction over exact source evidence.

use super::{Instance, OwnerId, ProductError, runtime};
use crate::{MemoryId, TrustedProjectSource};

pub use reliquary_application::{
    CompactionBudget, CompactionCheckpoint, CompactionPlan, ContextMessage, ContextRequest,
    ContextRole, ContextTokenCounter, ContextTurn, ContextView, MAX_CONTEXT_PROJECT_SOURCES,
    MAX_CONTEXT_SOURCE_BYTES, MAX_CONTEXT_SOURCE_TURNS,
};
pub use reliquary_context_engine::{
    CompactionError, DEFAULT_COMPACTION_TRIGGER_PERCENT, compaction_budget,
};
pub use runtime::{
    AuthorizedContextCheckpoint, AuthorizedContextWorkflow, ContextMemoryOwnerKind,
    ProjectContextAttachmentRef,
};

/// Select one current Memory from an explicitly granted REL or Phylactery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextMemoryRef {
    pub owner: OwnerId,
    pub kind: ContextMemoryOwnerKind,
    pub memory_id: MemoryId,
}

impl Instance {
    /// Prepare a host transcript with separately authorized Memory and project evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_context_workflow(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        native: ContextView,
        selected_memories: &[ContextMemoryRef],
        project_attachments: &[ProjectContextAttachmentRef],
        trusted_sources: &[&TrustedProjectSource],
        checkpoint: Option<&AuthorizedContextCheckpoint>,
        current_tokens: u64,
        budget: CompactionBudget,
        counter: &impl ContextTokenCounter,
    ) -> Result<AuthorizedContextWorkflow, ProductError> {
        self.require_rel_read(owner)?;
        if selected_memories.len() > MAX_CONTEXT_SOURCE_TURNS {
            return Err(ProductError::invalid("context source turn limit exceeded"));
        }
        let memories = selected_memories
            .iter()
            .map(|source| {
                match source.kind {
                    ContextMemoryOwnerKind::Rel => self.require_rel_read(source.owner)?,
                    ContextMemoryOwnerKind::Phy => self.require_phy_read(source.owner)?,
                }
                Ok(runtime::ContextMemoryRef {
                    owner: source.owner.into(),
                    kind: source.kind,
                    memory_id: source.memory_id.0,
                })
            })
            .collect::<Result<Vec<_>, ProductError>>()?;
        let sources = trusted_sources
            .iter()
            .map(|source| source.for_gateway(&self.gateway))
            .collect::<Result<Vec<_>, _>>()?;
        self.inner
            .prepare_context_workflow(
                owner.into(),
                conversation_id,
                native,
                &memories,
                project_attachments,
                &sources,
                checkpoint,
                current_tokens,
                budget,
                counter,
            )
            .map_err(Into::into)
    }

    pub fn context_request(
        &self,
        prepared: &AuthorizedContextWorkflow,
    ) -> Result<ContextRequest, ProductError> {
        self.inner.context_request(prepared).map_err(Into::into)
    }

    pub fn context_compaction_plan(
        &self,
        prepared: &AuthorizedContextWorkflow,
    ) -> Result<Option<CompactionPlan>, ProductError> {
        self.inner
            .context_compaction_plan(prepared)
            .map_err(Into::into)
    }

    pub fn finalize_context_workflow(
        &self,
        prepared: &AuthorizedContextWorkflow,
        summary: String,
        counter: &impl ContextTokenCounter,
    ) -> Result<Option<AuthorizedContextCheckpoint>, ProductError> {
        self.inner
            .finalize_context_workflow(prepared, summary, counter)
            .map_err(Into::into)
    }

    pub fn compact_context_with(
        &self,
        prepared: &AuthorizedContextWorkflow,
        infer: impl FnOnce(&CompactionPlan) -> Result<String, String>,
        counter: &impl ContextTokenCounter,
    ) -> Result<Option<AuthorizedContextCheckpoint>, ProductError> {
        self.inner
            .compact_context_with(prepared, infer, counter)
            .map_err(Into::into)
    }

    pub fn context_checkpoint(
        &self,
        checkpoint: &AuthorizedContextCheckpoint,
    ) -> Result<CompactionCheckpoint, ProductError> {
        self.inner
            .context_checkpoint(checkpoint)
            .map_err(Into::into)
    }
}
