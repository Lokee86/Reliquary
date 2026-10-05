use super::*;

impl GatewayRuntime {
    /// Cancel only the caller's own still-undispatched pending submission.
    /// Generation/host execution interruption is a separate shared-control API.
    pub fn cancel_conversation_submission(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::CancelOwnPendingSubmission)?;
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new(conversation_id.to_owned()).map_err(operation)?,
            submission_id: SubmissionId::new(submission_id.to_owned()).map_err(operation)?,
        };
        let journal = self.conversation.journal(owner_id)?;
        let status = with_conversation_admission(self, &token, || {
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let current = journal
                .status(&key)
                .ok_or_else(|| operation("submission not found"))?;
            if journal.submitter_principal_id(&key) != Some(instance.principal_id()) {
                return Err(operation(
                    "only the initiating principal may cancel its pending submission",
                ));
            }
            if current.state == SubmissionState::Cancelled {
                return Ok(current);
            }
            if current.state != SubmissionState::Pending {
                return Err(operation("only a pending submission may be cancelled"));
            }
            if current.generation.is_some()
                || journal
                    .host_operation(&key, RuntimeCapability::ConversationExecution)
                    .is_some()
            {
                return Err(operation(
                    "dispatched work must be interrupted through generation control",
                ));
            }
            compact_if_needed(&mut journal)?;
            let revision = journal.conversation_revision(&key.conversation_id);
            match journal.cancel(&key, revision).map_err(operation)? {
                TransitionOutcome::Durable(status) | TransitionOutcome::Idempotent(status) => {
                    Ok(status)
                }
            }
        })?;
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    /// Interrupt one already-dispatched generation/host operation by stable operation ID.
    /// This capability cannot cancel an undispatched queue item.
    pub fn cancel_conversation_generation(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        operation_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::CancelSharedGeneration)?;
        let conversation = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        if operation_id.trim().is_empty() {
            return Err(operation("generation operation ID must not be empty"));
        }
        let journal = self.conversation.journal(owner_id)?;

        let native = with_admission(self, &token, || {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Ok(journal.generation_by_operation_id(&conversation, operation_id))
        })?;
        if let Some((key, generation)) = native {
            let coordinator = self
                .conversation
                .coordinator(owner_id, conversation.as_str())?;
            let _processing = coordinator.acquire(&journal, &key, &generation.branch_id)?;
            let generation = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .generation(&key)
                .ok_or_else(|| operation("generation not found"))?;
            if generation.phase.terminal() {
                let status = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .status(&key)
                    .ok_or_else(|| operation("generation submission not found"))?;
                return Ok(GatewaySubmissionReceipt { id: key, status });
            }
            with_conversation_admission(self, &token, || {
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .record_generation_phase(&key, DurableGenerationPhase::CancellationRequested)
                    .map_err(operation)?;
                Ok(())
            })?;
            with_conversation_admission(self, &token, || {
                self.with_rel(&token, |runtime| {
                    interrupt_generation_stream(runtime, conversation.as_str(), &generation)
                })
            })?;
            let status = with_conversation_admission(self, &token, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                journal
                    .record_generation_phase(&key, DurableGenerationPhase::Interrupted)
                    .map_err(operation)?;
                journal
                    .status(&key)
                    .ok_or_else(|| operation("generation submission not found"))
            })?;
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }

        let host_match = with_admission(self, &token, || {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Ok(journal
                .host_operations()
                .into_iter()
                .find(|(key, capability, host)| {
                    key.conversation_id == conversation
                        && *capability == RuntimeCapability::ConversationExecution
                        && host.stable_id == operation_id
                }))
        })?;
        let Some((key, capability, host_operation)) = host_match else {
            return Err(operation("generation operation not found"));
        };
        let current = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        if host_operation.phase == DurableHostOperationPhase::Completed {
            return Ok(GatewaySubmissionReceipt {
                id: key,
                status: current,
            });
        }
        if host_operation.phase == DurableHostOperationPhase::Cancelled {
            let status = with_conversation_admission(self, &token, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                if journal
                    .status(&key)
                    .is_some_and(|status| status.state == SubmissionState::Pending)
                {
                    let revision = journal.conversation_revision(&key.conversation_id);
                    journal.cancel(&key, revision).map_err(operation)?;
                }
                journal
                    .status(&key)
                    .ok_or_else(|| operation("submission not found"))
            })?;
            if let Some(router) = self.host_router.as_ref() {
                router
                    .release_operation(capability, &host_operation.stable_id)
                    .map_err(operation)?;
            }
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }

        let Some(router) = self.host_router.as_ref() else {
            let status = with_conversation_admission(self, &token, || {
                let mut journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                if !host_operation.cancel_requested {
                    journal
                        .begin_host_cancel(&key, capability)
                        .map_err(operation)?;
                }
                journal
                    .record_host_status(&key, capability, HostOperationStatus::Unknown)
                    .map_err(operation)?;
                journal
                    .status(&key)
                    .ok_or_else(|| operation("submission not found"))
            })?;
            return Ok(GatewaySubmissionReceipt { id: key, status });
        };

        with_conversation_admission(self, &token, || {
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if !host_operation.cancel_requested {
                journal
                    .begin_host_cancel(&key, capability)
                    .map_err(operation)?;
            }
            Ok(())
        })?;
        let outcome = router.cancel(capability, &host_operation.stable_id);
        let status = with_conversation_admission(self, &token, || {
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            match outcome {
                Ok(outcome) => {
                    journal
                        .record_host_cancel(&key, capability, outcome)
                        .map_err(operation)?;
                    if matches!(
                        outcome,
                        HostCancelOutcome::CancelledBeforeStart
                            | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Cancelled)
                    ) {
                        let revision = journal.conversation_revision(&key.conversation_id);
                        journal.cancel(&key, revision).map_err(operation)?;
                    }
                }
                Err(_) => {
                    journal
                        .record_host_status(&key, capability, HostOperationStatus::Unknown)
                        .map_err(operation)?;
                }
            }
            journal
                .status(&key)
                .ok_or_else(|| operation("submission not found"))
        })?;
        if matches!(
            outcome,
            Ok(HostCancelOutcome::CancelledBeforeStart
                | HostCancelOutcome::AlreadyTerminal(
                    HostOperationStatus::Cancelled | HostOperationStatus::Completed
                ))
        ) {
            router
                .release_operation(capability, &host_operation.stable_id)
                .map_err(operation)?;
        }
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    /// Start the one durable generated reply owned by a committed user turn.
    /// This trusted owner path is independent of the submitting/viewing instance lifetime.
    pub fn start_conversation_generation(
        &self,
        owner_id: &str,
        conversation_id: &str,
        submission_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let key = ConversationSubmissionId {
            conversation_id: ConversationId::new(conversation_id.to_owned()).map_err(operation)?,
            submission_id: SubmissionId::new(submission_id.to_owned()).map_err(operation)?,
        };
        let journal = self.conversation.journal(owner_id)?;
        let (status, receipt) = {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let status = journal
                .status(&key)
                .ok_or_else(|| operation("submission not found"))?;
            if status.state != SubmissionState::Committed || status.post_commit_receipt.is_none() {
                return Err(operation(
                    "generation requires a fully committed conversation submission",
                ));
            }
            let receipt = status
                .archive_receipt
                .clone()
                .ok_or_else(|| operation("committed submission lacks Archive receipt"))?;
            (status, receipt)
        };
        if status
            .generation
            .as_ref()
            .is_some_and(|generation| generation.phase.terminal())
        {
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }

        let (operation_id, generation_id) = conversation_generation_identity(owner_id, &key);
        let branch_id = receipt.branch_id.clone();
        let coordinator = self
            .conversation
            .coordinator(owner_id, key.conversation_id.as_str())?;
        let _processing = coordinator.acquire(&journal, &key, &branch_id)?;
        let status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        if status
            .generation
            .as_ref()
            .is_some_and(|generation| generation.phase.terminal())
        {
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }
        with_operation(self, &authority, |runtime| {
            validate_archive_branch(
                runtime,
                key.conversation_id.as_str(),
                &branch_id,
                &receipt.node_id,
            )
        })?;

        let generation = if let Some(generation) = status.generation.clone() {
            if generation.operation_id != operation_id
                || generation.generation_id != generation_id
                || generation.branch_id != branch_id
                || generation.parent_node_id != receipt.node_id
            {
                return Err(operation(
                    "generation identity conflicts with durable state",
                ));
            }
            generation
        } else {
            let started_timestamp_ns = now_ns();
            with_accepted_work(self, &authority, || {
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .begin_generation(
                        &key,
                        operation_id.clone(),
                        generation_id.clone(),
                        branch_id.clone(),
                        receipt.node_id.clone(),
                        started_timestamp_ns,
                        DurableGenerationPhase::Pending,
                    )
                    .map_err(operation)?;
                Ok(())
            })?;
            journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .generation(&key)
                .ok_or_else(|| operation("durable generation was not recorded"))?
        };

        if generation.phase == DurableGenerationPhase::Pending {
            let initiating_generation_id = conversation_host_operation_id(owner_id, &key);
            with_accepted_work(self, &authority, || {
                self.conversation
                    .owner_journal(owner_id)?
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .bind_agent_source(
                        &initiating_generation_id,
                        &generation.generation_id,
                        &generation.generation_id,
                    )
                    .map_err(operation)?;
                Ok(())
            })?;
            with_operation(self, &authority, |runtime| {
                validate_archive_branch(
                    runtime,
                    key.conversation_id.as_str(),
                    &generation.branch_id,
                    &generation.parent_node_id,
                )?;
                persist_generation_stream(
                    runtime,
                    key.conversation_id.as_str(),
                    &generation,
                    "",
                    InteractionStreamStatus::Streaming,
                )
            })?;
            with_accepted_work(self, &authority, || {
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .record_generation_phase(&key, DurableGenerationPhase::Running)
                    .map_err(operation)?;
                Ok(())
            })?;
        }

        let status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    /// Durably checkpoint a full generated-text prefix. The interaction-stream
    /// record is synchronized before the journal exposes the checkpoint sequence.
    pub fn checkpoint_conversation_generation(
        &self,
        owner_id: &str,
        conversation_id: &str,
        operation_id: &str,
        checkpoint_sequence: u64,
        content_prefix: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let conversation = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        let (key, generation) = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .generation_by_operation_id(&conversation, operation_id)
            .ok_or_else(|| operation("generation operation not found"))?;
        let coordinator = self
            .conversation
            .coordinator(owner_id, conversation.as_str())?;
        let _processing = coordinator.acquire(&journal, &key, &generation.branch_id)?;
        let generation = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .generation(&key)
            .ok_or_else(|| operation("generation not found"))?;
        if generation.phase != DurableGenerationPhase::Running {
            return Err(operation("generation is not running"));
        }
        let digest: [u8; 32] = Sha256::digest(content_prefix.as_bytes()).into();
        let bytes = u64::try_from(content_prefix.len())
            .map_err(|_| operation("generation checkpoint length is exhausted"))?;
        if let Some(current) = generation.checkpoint.as_ref() {
            if checkpoint_sequence == current.sequence {
                if current.utf8_bytes != bytes || current.sha256 != digest {
                    return Err(operation(
                        "checkpoint sequence reused with different content",
                    ));
                }
                with_operation(self, &authority, |runtime| {
                    validate_generation_stream(
                        runtime,
                        conversation.as_str(),
                        &generation,
                        content_prefix,
                    )
                })?;
                let status = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .status(&key)
                    .ok_or_else(|| operation("submission not found"))?;
                return Ok(GatewaySubmissionReceipt { id: key, status });
            }
            if checkpoint_sequence
                != current
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| operation("generation checkpoint sequence is exhausted"))?
            {
                return Err(operation("generation checkpoint sequence is not next"));
            }
        } else if checkpoint_sequence != 1 {
            return Err(operation(
                "first generation checkpoint sequence must be one",
            ));
        }

        with_operation(self, &authority, |runtime| {
            if let Some(current) =
                generation_stream_for_conversation(runtime, conversation.as_str(), &generation)?
            {
                if !content_prefix.starts_with(&current.content) {
                    return Err(operation(
                        "checkpoint must extend the durable content prefix",
                    ));
                }
            }
            persist_generation_stream(
                runtime,
                conversation.as_str(),
                &generation,
                content_prefix,
                InteractionStreamStatus::Streaming,
            )
        })?;
        with_accepted_work(self, &authority, || {
            journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .record_generation_checkpoint(&key, checkpoint_sequence, bytes, digest)
                .map_err(operation)?;
            Ok(())
        })?;
        let status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    /// Commit the synchronized generated prefix as one stable assistant turn.
    pub fn complete_conversation_generation(
        &self,
        owner_id: &str,
        conversation_id: &str,
        operation_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let authority = OperationAuthority::capture(self, owner_id)?;
        let conversation = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        let (key, generation, principal_id) = {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let (key, generation) = journal
                .generation_by_operation_id(&conversation, operation_id)
                .ok_or_else(|| operation("generation operation not found"))?;
            let principal_id = journal
                .submitter_principal_id(&key)
                .ok_or_else(|| operation("generation submitter identity is unavailable"))?
                .to_owned();
            (key, generation, principal_id)
        };
        let coordinator = self
            .conversation
            .coordinator(owner_id, conversation.as_str())?;
        let _processing = coordinator.acquire(&journal, &key, &generation.branch_id)?;
        let generation = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .generation(&key)
            .ok_or_else(|| operation("generation not found"))?;
        if generation.phase == DurableGenerationPhase::Completed {
            let status = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .status(&key)
                .ok_or_else(|| operation("submission not found"))?;
            return Ok(GatewaySubmissionReceipt { id: key, status });
        }
        if !matches!(
            generation.phase,
            DurableGenerationPhase::Running | DurableGenerationPhase::CancellationRequested
        ) {
            return Err(operation(
                "generation cannot be completed from its current state",
            ));
        }

        with_operation(self, &authority, |runtime| {
            let record =
                generation_stream_for_conversation(runtime, conversation.as_str(), &generation)?
                    .ok_or_else(|| operation("generation has no durable stream record"))?;
            validate_generation_stream_record(&record, &generation)?;
            if record.status != InteractionStreamStatus::Streaming {
                return Err(operation("interrupted generation cannot be committed"));
            }
            let digest: [u8; 32] = Sha256::digest(record.content.as_bytes()).into();
            let bytes = u64::try_from(record.content.len())
                .map_err(|_| operation("generation content length is exhausted"))?;
            match generation.checkpoint.as_ref() {
                Some(checkpoint)
                    if checkpoint.utf8_bytes == bytes && checkpoint.sha256 == digest => {}
                None if record.content.is_empty() => {}
                _ => {
                    return Err(operation(
                        "generation content is not covered by the last durable checkpoint",
                    ));
                }
            }
            let branch_leaf = runtime
                .cva()
                .archive()
                .branches()
                .iter()
                .find(|branch| {
                    branch.conversation_id == conversation.as_str()
                        && branch.id == generation.branch_id
                })
                .map(|branch| branch.leaf_node_id.clone())
                .ok_or_else(|| operation("generation branch is absent from Archive"))?;
            let node_exists = runtime
                .cva()
                .archive()
                .has_node(conversation.as_str(), &generation.generation_id);
            if node_exists {
                validate_generation_archive_turn(
                    runtime,
                    conversation.as_str(),
                    &generation,
                    &principal_id,
                    &record.content,
                )?;
                if branch_leaf == generation.parent_node_id {
                    publish_branch(
                        runtime,
                        conversation.as_str(),
                        &generation.branch_id,
                        &generation.generation_id,
                        true,
                    )?;
                } else if branch_leaf != generation.generation_id {
                    return Err(operation(
                        "generation branch advanced outside the active generation",
                    ));
                } else {
                    runtime.cva().sync().map_err(operation)?;
                }
            } else {
                if branch_leaf != generation.parent_node_id {
                    return Err(operation(
                        "generation branch advanced before the generated turn committed",
                    ));
                }
                runtime
                    .cva
                    .append_node_with_principal(
                        generation.generation_id.clone(),
                        conversation.as_str().to_owned(),
                        Some(generation.parent_node_id.clone()),
                        "assistant".to_owned(),
                        Some(principal_id.clone()),
                        generation.started_timestamp_ns,
                        &record.content,
                    )
                    .map_err(operation)?;
                runtime.cva().sync().map_err(operation)?;
                publish_branch(
                    runtime,
                    conversation.as_str(),
                    &generation.branch_id,
                    &generation.generation_id,
                    true,
                )?;
            }
            validate_archive_branch(
                runtime,
                conversation.as_str(),
                &generation.branch_id,
                &generation.generation_id,
            )?;
            validate_generation_archive_turn(
                runtime,
                conversation.as_str(),
                &generation,
                &principal_id,
                &record.content,
            )?;
            ensure_session_at(
                runtime,
                conversation.as_str(),
                Some(&generation.generation_id),
            )?;
            runtime
                .schedule_session(conversation.as_str(), self.episode_policy, now_ns())
                .map_err(operation)?;
            Ok(())
        })?;
        with_accepted_work(self, &authority, || {
            journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .record_generation_phase(&key, DurableGenerationPhase::Completed)
                .map_err(operation)?;
            Ok(())
        })?;
        let status = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .status(&key)
            .ok_or_else(|| operation("submission not found"))?;
        Ok(GatewaySubmissionReceipt { id: key, status })
    }

    /// Minimal reconnect seam for P3.6. P3.7 builds the full authoritative snapshot.
    pub fn conversation_generation_status(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
        operation_id: &str,
    ) -> Result<GatewaySubmissionReceipt, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        with_admission(self, &token, || {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let (key, _) = journal
                .generation_by_operation_id(&conversation, operation_id)
                .ok_or_else(|| operation("generation operation not found"))?;
            let status = journal
                .status(&key)
                .ok_or_else(|| operation("generation submission not found"))?;
            Ok(GatewaySubmissionReceipt { id: key, status })
        })
    }
}

fn conversation_generation_identity(
    owner_id: &str,
    key: &ConversationSubmissionId,
) -> (String, String) {
    let mut digest = Sha256::new();
    for component in [
        "native-agent-generation",
        owner_id,
        key.conversation_id.as_str(),
        key.submission_id.as_str(),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    let suffix = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (
        format!("reliquary/conversation-generation/{suffix}"),
        format!("generation-{suffix}"),
    )
}

pub(super) fn generation_stream_for_conversation(
    runtime: &crate::InteractionRuntime,
    conversation_id: &str,
    generation: &DurableGeneration,
) -> Result<Option<InteractionStreamRecord>, ReliquaryRuntimeHostError> {
    let record = runtime
        .cva()
        .interaction_streams_for_session(conversation_id)
        .into_iter()
        .find(|record| record.message_id == generation.generation_id);
    if let Some(record) = record.as_ref() {
        validate_generation_stream_record(record, generation)?;
    }
    Ok(record)
}

fn validate_generation_stream_record(
    record: &InteractionStreamRecord,
    generation: &DurableGeneration,
) -> Result<(), ReliquaryRuntimeHostError> {
    if record.message_id != generation.generation_id
        || record.parent_message_id.as_deref() != Some(generation.parent_node_id.as_str())
        || record.role != InteractionRole::Agent
        || record.timestamp_ns != generation.started_timestamp_ns
    {
        return Err(operation(
            "generation interaction-stream identity differs from durable generation",
        ));
    }
    Ok(())
}

fn validate_generation_stream(
    runtime: &crate::InteractionRuntime,
    conversation_id: &str,
    generation: &DurableGeneration,
    content: &str,
) -> Result<(), ReliquaryRuntimeHostError> {
    let record = generation_stream_for_conversation(runtime, conversation_id, generation)?
        .ok_or_else(|| operation("generation has no durable stream record"))?;
    if record.content != content || record.status != InteractionStreamStatus::Streaming {
        return Err(operation(
            "generation stream differs from the durable checkpoint content",
        ));
    }
    Ok(())
}

fn persist_generation_stream(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    generation: &DurableGeneration,
    content: &str,
    status: InteractionStreamStatus,
) -> Result<(), ReliquaryRuntimeHostError> {
    if let Some(current) = generation_stream_for_conversation(runtime, conversation_id, generation)?
    {
        if current.content == content && current.status == status {
            return runtime.cva().sync().map_err(operation);
        }
    }
    runtime
        .cva
        .put_interaction_stream(InteractionStreamRecord {
            message_id: generation.generation_id.clone(),
            session_id: conversation_id.to_owned(),
            parent_message_id: Some(generation.parent_node_id.clone()),
            role: InteractionRole::Agent,
            timestamp_ns: generation.started_timestamp_ns,
            content: content.to_owned(),
            status,
        })
        .map_err(operation)?;
    runtime.cva().sync().map_err(operation)
}

pub(super) fn interrupt_generation_stream(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    generation: &DurableGeneration,
) -> Result<(), ReliquaryRuntimeHostError> {
    let content = match generation_stream_for_conversation(runtime, conversation_id, generation)? {
        Some(record) => record.content,
        None if generation.checkpoint.is_none() => String::new(),
        None => {
            return Err(operation(
                "durable generation checkpoint is missing its interaction stream",
            ));
        }
    };
    persist_generation_stream(
        runtime,
        conversation_id,
        generation,
        &content,
        InteractionStreamStatus::Interrupted,
    )
}

pub(super) fn validate_generation_archive_turn(
    runtime: &mut crate::InteractionRuntime,
    conversation_id: &str,
    generation: &DurableGeneration,
    principal_id: &str,
    content: &str,
) -> Result<(), ReliquaryRuntimeHostError> {
    let node = runtime
        .cva()
        .archive()
        .nodes
        .get(conversation_id, &generation.generation_id)
        .ok_or_else(|| operation("generated Archive node is absent"))?;
    let digest: [u8; 32] = Sha256::digest(content.as_bytes()).into();
    if node.conversation_id != conversation_id
        || node.parent_id.as_deref() != Some(generation.parent_node_id.as_str())
        || node.role != "assistant"
        || node.principal_id.as_deref() != Some(principal_id)
        || node.timestamp_ns != generation.started_timestamp_ns
        || node.content_id.0 != digest
    {
        return Err(operation(
            "generated Archive node differs from durable generation identity",
        ));
    }
    let turn = runtime
        .conversation_turns(conversation_id, &generation.generation_id)
        .map_err(operation)?
        .into_iter()
        .find(|turn| turn.node_id == generation.generation_id)
        .ok_or_else(|| operation("generated Archive turn is unavailable"))?;
    if turn.content != content || turn.role != "assistant" {
        return Err(operation(
            "generated Archive turn differs from durable checkpoint content",
        ));
    }
    Ok(())
}
