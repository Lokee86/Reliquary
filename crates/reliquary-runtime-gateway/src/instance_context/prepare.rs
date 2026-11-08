use super::*;

impl Instance {
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
    ) -> Result<AuthorizedContextWorkflow, GatewayError> {
        self.with_reliquary(owner, |_| ())?;
        if native.turns.len() > reliquary_application::MAX_CONTEXT_SOURCE_TURNS
            || selected_memories.len() > reliquary_application::MAX_CONTEXT_SOURCE_TURNS
        {
            return Err(GatewayError::ProjectSource(
                "context source turn limit exceeded".into(),
            ));
        }
        let native_digest = context_view_digest(&native);
        let native_view = native.clone();
        if project_attachments.len() > reliquary_application::MAX_CONTEXT_PROJECT_SOURCES {
            return Err(GatewayError::ProjectSource(
                "too many project context sources".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        let descriptors = self.read_context_attachments(owner, project_attachments)?;
        let mut source_bytes = native
            .turns
            .iter()
            .try_fold(0usize, |total, turn| total.checked_add(turn.content.len()))
            .ok_or_else(|| GatewayError::ProjectSource("context source size overflow".into()))?;
        let mut evidence = Vec::with_capacity(project_attachments.len());
        let mut sources = Vec::with_capacity(project_attachments.len());
        let mut memory_evidence = Vec::with_capacity(selected_memories.len());
        let mut memories = Vec::with_capacity(selected_memories.len());
        let mut seen_memories = BTreeSet::new();
        for source in selected_memories {
            if !seen_memories.insert((source.owner, source.kind, source.memory_id)) {
                return Err(GatewayError::ProjectSource(
                    "duplicate selected Memory source".into(),
                ));
            }
            let (memory, binding) = self.read_context_memory(*source)?;
            source_bytes = source_bytes
                .checked_add(memory.title.len())
                .and_then(|n| n.checked_add(memory.content.len()))
                .ok_or_else(|| {
                    GatewayError::ProjectSource("context source size overflow".into())
                })?;
            if source_bytes > reliquary_application::MAX_CONTEXT_SOURCE_BYTES {
                return Err(GatewayError::ProjectSource(
                    "context source byte limit exceeded".into(),
                ));
            }
            memory_evidence.push(memory);
            memories.push(binding);
        }
        for (selector, attachment) in project_attachments.iter().zip(descriptors.iter()) {
            if !seen.insert((
                selector.conversation_id.clone(),
                selector.source_node_id.clone(),
                selector.attachment_id,
            )) {
                return Err(GatewayError::ProjectSource(
                    "duplicate project context source".into(),
                ));
            }
            let reference = attachment.project_reference.clone().ok_or_else(|| {
                GatewayError::ProjectSource("Archive attachment is not a project reference".into())
            })?;
            if attachment.id != selector.attachment_id
                || reference.content_hash.is_none()
                || !attachment.bytes.is_empty()
                || Some(attachment.content_id.0) != reference.content_hash
            {
                return Err(GatewayError::ProjectSource(
                    "Archive project source identity is incomplete".into(),
                ));
            }
            source_bytes = source_bytes
                .checked_add(usize::try_from(attachment.byte_length).unwrap_or(usize::MAX))
                .ok_or_else(|| {
                    GatewayError::ProjectSource("context source size overflow".into())
                })?;
            if source_bytes > reliquary_application::MAX_CONTEXT_SOURCE_BYTES {
                return Err(GatewayError::ProjectSource(
                    "context source byte limit exceeded".into(),
                ));
            }
            let (root, active) = trusted_sources
                .iter()
                .find_map(|source| {
                    source
                        .context_root(self, owner, &reference.revision.repository)
                        .ok()
                })
                .ok_or_else(|| {
                    GatewayError::ProjectSource(
                        "no trusted project source matches the Archive reference".into(),
                    )
                })?;
            let bytes = reliquary_application::read_context_repository_source(&root, &reference)
                .map_err(|error| GatewayError::ProjectSource(error.to_string()))?;
            if !active.load(Ordering::Acquire) {
                return Err(GatewayError::ProjectSource(
                    "project source authorization changed".into(),
                ));
            }
            evidence.push(ContextRepositoryEvidence {
                attachment_id: attachment.id,
                reference: reference.clone(),
                byte_length: attachment.byte_length,
                bytes,
            });
            sources.push(AuthorizedProjectContextSource {
                selector: selector.clone(),
                descriptor: attachment.clone(),
                active,
            });
        }
        self.authorize_queue_owner(owner)?;
        if self.read_context_attachments(owner, project_attachments)? != descriptors {
            return Err(GatewayError::ProjectSource(
                "Archive project evidence changed during hydration".into(),
            ));
        }
        if let Some(checkpoint) = checkpoint {
            // The caller's counter sees the summary while measuring the request.
            self.validate_context_checkpoint(checkpoint, owner, conversation_id, &native)?;
        }
        let prepared = prepare_context_workflow(
            native,
            evidence,
            memory_evidence,
            checkpoint.map(|checkpoint| &checkpoint.checkpoint),
            current_tokens,
            budget,
            counter,
        )
        .map_err(|error| GatewayError::ProjectSource(error.to_string()))?;
        let source_view = prepared.source_view().clone();
        if let Some(checkpoint) = checkpoint {
            self.validate_context_checkpoint(checkpoint, owner, conversation_id, &source_view)?;
        }
        let prepared = AuthorizedContextWorkflow {
            instance_id: self.instance_id,
            principal: self.principal.to_string(),
            owner,
            conversation_id: conversation_id.to_owned(),
            native_view,
            native_digest,
            source_digest: context_view_digest(&source_view),
            source_view,
            prepared,
            sources,
            memories,
        };
        self.validate_context_workflow_scope(&prepared)?;
        Ok(prepared)
    }
}
