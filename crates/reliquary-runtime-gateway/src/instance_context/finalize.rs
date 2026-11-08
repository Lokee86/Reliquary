use super::*;

impl Instance {
    /// Reauthorize the exact captured owner and project cut after inference,
    /// then validate citations and normalize output to the planned budget.
    pub fn finalize_context_workflow(
        &self,
        prepared: &AuthorizedContextWorkflow,
        summary: String,
        counter: &impl ContextTokenCounter,
    ) -> Result<Option<AuthorizedContextCheckpoint>, GatewayError> {
        self.validate_context_workflow_scope(prepared)?;
        let checkpoint = finalize_context_workflow(&prepared.prepared, summary, counter);
        self.validate_context_workflow_scope(prepared)?;
        checkpoint
            .map(|checkpoint| {
                checkpoint.map(|checkpoint| AuthorizedContextCheckpoint {
                    instance_id: prepared.instance_id,
                    principal: prepared.principal.clone(),
                    owner: prepared.owner,
                    conversation_id: prepared.conversation_id.clone(),
                    checkpoint,
                    native_view: prepared.native_view.clone(),
                    native_digest: prepared.native_digest,
                    source_view: prepared.source_view.clone(),
                    source_digest: prepared.source_digest,
                    sources: prepared.sources.clone(),
                    memories: prepared.memories.clone(),
                })
            })
            .map_err(|error| GatewayError::ProjectSource(error.to_string()))
    }

    pub fn context_request(
        &self,
        prepared: &AuthorizedContextWorkflow,
    ) -> Result<ContextRequest, GatewayError> {
        self.validate_context_workflow_scope(prepared)?;
        Ok(prepared.request().clone())
    }

    pub fn context_compaction_plan(
        &self,
        prepared: &AuthorizedContextWorkflow,
    ) -> Result<Option<CompactionPlan>, GatewayError> {
        self.validate_context_workflow_scope(prepared)?;
        Ok(prepared.compaction_plan().cloned())
    }

    pub fn context_checkpoint(
        &self,
        checkpoint: &AuthorizedContextCheckpoint,
    ) -> Result<CompactionCheckpoint, GatewayError> {
        self.with_reliquary(checkpoint.owner, |_| ())?;
        self.validate_context_checkpoint(
            checkpoint,
            checkpoint.owner,
            &checkpoint.conversation_id,
            &checkpoint.source_view,
        )?;
        Ok(checkpoint.checkpoint.clone())
    }

    pub(super) fn validate_context_workflow_scope(
        &self,
        prepared: &AuthorizedContextWorkflow,
    ) -> Result<(), GatewayError> {
        if prepared.instance_id != self.instance_id || prepared.principal != self.principal.as_ref()
        {
            return Err(GatewayError::ProjectSource(
                "context belongs to another Instance".into(),
            ));
        }
        self.with_reliquary(prepared.owner, |_| ())?;
        if context_view_digest(&prepared.native_view) != prepared.native_digest {
            return Err(GatewayError::ProjectSource(
                "native host context snapshot changed".into(),
            ));
        }
        if context_view_digest(&prepared.source_view) != prepared.source_digest {
            return Err(GatewayError::ProjectSource(
                "context source snapshot is invalid".into(),
            ));
        }
        for binding in &prepared.memories {
            let (_, current) = self.read_context_memory(binding.source)?;
            if current != *binding {
                return Err(GatewayError::ProjectSource(
                    "selected Memory source changed during inference".into(),
                ));
            }
        }
        if prepared
            .sources
            .iter()
            .any(|source| !source.active.load(Ordering::Acquire))
        {
            return Err(GatewayError::ProjectSource(
                "project source authorization changed".into(),
            ));
        }
        let selectors = prepared
            .sources
            .iter()
            .map(|source| source.selector.clone())
            .collect::<Vec<_>>();
        let descriptors = self.read_context_attachments(prepared.owner, &selectors)?;
        if descriptors.len() != prepared.sources.len()
            || descriptors
                .iter()
                .zip(&prepared.sources)
                .any(|(descriptor, source)| descriptor != &source.descriptor)
        {
            return Err(GatewayError::ProjectSource(
                "context source cut changed during inference".into(),
            ));
        }
        Ok(())
    }

    /// Invoke provider inference with no Gateway owner lock held, then bind its
    /// output back to the captured source scope before accepting a checkpoint.
    pub fn compact_context_with(
        &self,
        prepared: &AuthorizedContextWorkflow,
        infer: impl FnOnce(&CompactionPlan) -> Result<String, String>,
        counter: &impl ContextTokenCounter,
    ) -> Result<Option<AuthorizedContextCheckpoint>, GatewayError> {
        self.validate_context_workflow_scope(prepared)?;
        let Some(plan) = prepared.compaction_plan() else {
            return Ok(None);
        };
        let summary = infer(plan).map_err(GatewayError::ProjectSource)?;
        self.finalize_context_workflow(prepared, summary, counter)
    }

    pub(super) fn validate_context_checkpoint(
        &self,
        checkpoint: &AuthorizedContextCheckpoint,
        owner: OwnerId,
        conversation_id: &str,
        current_view: &ContextView,
    ) -> Result<(), GatewayError> {
        if checkpoint.instance_id != self.instance_id
            || checkpoint.principal != self.principal.as_ref()
            || checkpoint.owner != owner
            || checkpoint.conversation_id != conversation_id
        {
            return Err(GatewayError::ProjectSource(
                "context checkpoint belongs to another scope".into(),
            ));
        }
        if context_view_digest(&checkpoint.native_view) != checkpoint.native_digest {
            return Err(GatewayError::ProjectSource(
                "context checkpoint host snapshot is invalid".into(),
            ));
        }
        if context_view_digest(&checkpoint.source_view) != checkpoint.source_digest {
            return Err(GatewayError::ProjectSource(
                "context checkpoint source snapshot is invalid".into(),
            ));
        }
        if let Some(old_end) = checkpoint
            .source_view
            .turns
            .iter()
            .position(|turn| turn.message_id == checkpoint.checkpoint.through_message_id)
        {
            let old_prefix = &checkpoint.source_view.turns[..=old_end];
            if current_view.turns.len() > old_end {
                let current_prefix = &current_view.turns[..=old_end];
                if old_prefix != current_prefix {
                    return Err(GatewayError::ProjectSource(
                        "host transcript changed within compacted checkpoint prefix".into(),
                    ));
                }
            }
        }
        if checkpoint
            .sources
            .iter()
            .any(|source| !source.active.load(Ordering::Acquire))
        {
            return Err(GatewayError::ProjectSource(
                "context checkpoint project grant was revoked".into(),
            ));
        }
        for binding in &checkpoint.memories {
            let (_, current) = self.read_context_memory(binding.source)?;
            if current != *binding {
                return Err(GatewayError::ProjectSource(
                    "context checkpoint Memory source changed".into(),
                ));
            }
        }
        let selectors = checkpoint
            .sources
            .iter()
            .map(|source| source.selector.clone())
            .collect::<Vec<_>>();
        let current = self.read_context_attachments(owner, &selectors)?;
        if current.len() != checkpoint.sources.len()
            || current
                .iter()
                .zip(&checkpoint.sources)
                .any(|(now, source)| now != &source.descriptor)
        {
            return Err(GatewayError::ProjectSource(
                "context checkpoint project source changed".into(),
            ));
        }
        Ok(())
    }
}
