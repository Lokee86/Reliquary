use super::*;

impl GatewayRuntime {
    /// Recover native journal work before the REL runtime's workers are started.
    /// Pending entries are deliberately left queued; Finalizing entries use their
    /// captured stable node ID and Committed entries reconcile the shared head.
    pub(in super::super) fn recover_rel_before_start(
        &self,
        owner_id: &str,
        runtime: &mut crate::InteractionRuntime,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(owner_id)?;
        // Validate branch ownership before recovery can publish any effects.
        self.conversation
            .rebuild_continuations(owner_id, runtime.cva().archive().branches())?;
        let durable_host_operations = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .host_operations();
        if let Some(router) = self.host_router.as_ref() {
            for (_, capability, host_operation) in durable_host_operations {
                if !matches!(
                    host_operation.phase,
                    DurableHostOperationPhase::Completed | DurableHostOperationPhase::Cancelled
                ) {
                    router
                        .hold_operation(capability, host_operation.stable_id)
                        .map_err(operation)?;
                }
            }
        }
        recover_journal_runtime(&journal, owner_id, runtime, self.episode_policy)?;
        self.conversation
            .rebuild_continuations(owner_id, runtime.cva().archive().branches())
    }

    /// Reconcile existing publication while quiesced; never start queued work.
    pub(in super::super) fn reconcile_rel(
        &self,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(owner_id)?;
        with_internal_runtime(self, owner_id, |runtime| {
            compact_if_needed(
                &mut *journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?,
            )?;
            recover_submission_receipts(&journal, owner_id, runtime, self.episode_policy)?;
            compact_if_needed(
                &mut *journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?,
            )
        })
    }
}

fn recover_journal_runtime(
    journal: &Arc<Mutex<RelJournal>>,
    owner_id: &str,
    runtime: &mut crate::InteractionRuntime,
    episode_policy: crate::EpisodePolicy,
) -> Result<(), ReliquaryRuntimeHostError> {
    {
        let mut journal = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        compact_if_needed(&mut journal)?;
    }
    let statuses = journal
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .statuses();
    for (key, status) in statuses {
        if let Some(generation) = status
            .generation
            .as_ref()
            .filter(|generation| !generation.phase.terminal())
        {
            reconcile_generation_checkpoint(runtime, journal, &key, generation)?;
            if runtime
                .cva()
                .archive()
                .has_node(key.conversation_id.as_str(), &generation.generation_id)
            {
                let principal_id = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .submitter_principal_id(&key)
                    .ok_or_else(|| operation("generation submitter identity is unavailable"))?
                    .to_owned();
                let record = generation_stream_for_conversation(
                    runtime,
                    key.conversation_id.as_str(),
                    generation,
                )?
                .ok_or_else(|| operation("committed generation lacks its stream checkpoint"))?;
                validate_generation_archive_turn(
                    runtime,
                    key.conversation_id.as_str(),
                    generation,
                    &principal_id,
                    &record.content,
                )?;
                let branch_leaf = runtime
                    .cva()
                    .archive()
                    .branches()
                    .iter()
                    .find(|branch| {
                        branch.conversation_id == key.conversation_id.as_str()
                            && branch.id == generation.branch_id
                    })
                    .map(|branch| branch.leaf_node_id.clone())
                    .ok_or_else(|| operation("generation recovery branch is missing"))?;
                if branch_leaf == generation.parent_node_id {
                    publish_branch(
                        runtime,
                        key.conversation_id.as_str(),
                        &generation.branch_id,
                        &generation.generation_id,
                        true,
                    )?;
                }
                validate_archive_branch(
                    runtime,
                    key.conversation_id.as_str(),
                    &generation.branch_id,
                    &generation.generation_id,
                )?;
                ensure_session_at(
                    runtime,
                    key.conversation_id.as_str(),
                    Some(&generation.generation_id),
                )?;
                runtime
                    .schedule_session(key.conversation_id.as_str(), episode_policy, now_ns())
                    .map_err(operation)?;
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .record_generation_phase(&key, DurableGenerationPhase::Completed)
                    .map_err(operation)?;
            } else {
                // Native generation execution is process-local. A restart can recover
                // only the synchronized prefix, never infer that compute is still live.
                interrupt_generation_stream(runtime, key.conversation_id.as_str(), generation)?;
                journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .record_generation_phase(&key, DurableGenerationPhase::Interrupted)
                    .map_err(operation)?;
            }
        }
    }
    recover_submission_receipts(journal, owner_id, runtime, episode_policy)
}

/// A stream may be one synchronized append ahead of its journal marker.
/// Verify the acknowledged UTF-8 prefix before accepting that crash handoff.
fn reconcile_generation_checkpoint(
    runtime: &crate::InteractionRuntime,
    journal: &Arc<Mutex<RelJournal>>,
    key: &ConversationSubmissionId,
    generation: &DurableGeneration,
) -> Result<(), ReliquaryRuntimeHostError> {
    let Some(record) =
        generation_stream_for_conversation(runtime, key.conversation_id.as_str(), generation)?
    else {
        if generation.checkpoint.is_some() {
            return Err(operation(
                "durable generation checkpoint is missing its interaction stream",
            ));
        }
        return Ok(());
    };
    let bytes = u64::try_from(record.content.len())
        .map_err(|_| operation("generation checkpoint length exhausted"))?;
    let hash: [u8; 32] = Sha256::digest(record.content.as_bytes()).into();
    if let Some(checkpoint) = generation.checkpoint.as_ref() {
        let length = usize::try_from(checkpoint.utf8_bytes)
            .map_err(|_| operation("generation checkpoint length exhausted"))?;
        let prefix = record
            .content
            .get(..length)
            .ok_or_else(|| operation("recovered stream lost its durable checkpoint prefix"))?;
        let prefix_hash: [u8; 32] = Sha256::digest(prefix.as_bytes()).into();
        if prefix_hash != checkpoint.sha256 {
            return Err(operation(
                "recovered stream changed its durable checkpoint prefix",
            ));
        }
    }
    let differs = generation
        .checkpoint
        .as_ref()
        .map_or(!record.content.is_empty(), |checkpoint| {
            checkpoint.utf8_bytes != bytes || checkpoint.sha256 != hash
        });
    if differs {
        let sequence = generation.checkpoint.as_ref().map_or(Ok(1), |checkpoint| {
            checkpoint
                .sequence
                .checked_add(1)
                .ok_or_else(|| operation("generation checkpoint sequence exhausted"))
        })?;
        journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .record_generation_checkpoint(key, sequence, bytes, hash)
            .map_err(operation)?;
    }
    Ok(())
}

fn recover_submission_receipts(
    journal: &Arc<Mutex<RelJournal>>,
    owner_id: &str,
    runtime: &mut crate::InteractionRuntime,
    episode_policy: crate::EpisodePolicy,
) -> Result<(), ReliquaryRuntimeHostError> {
    let statuses = journal
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .statuses();
    for (key, status) in statuses {
        if status.state == SubmissionState::Finalizing {
            let (capture, submission) = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let capture = status
                    .finalize_capture
                    .clone()
                    .ok_or_else(|| operation("finalizing entry lacks captured turn"))?;
                let submission = journal
                    .submission(&key)
                    .cloned()
                    .ok_or_else(|| operation("finalizing entry payload is not retained"))?;
                (capture, submission)
            };
            validate_accepted_submission(owner_id, &key, &submission)?;
            // Startup reconciles published turns. Missing publication remains
            // queued for the owner coordinator, independently of attachments.
            let Some(receipt) = archive_receipt_if_present(runtime, &key, &submission, &capture)?
            else {
                continue;
            };
            let mut journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            journal
                .record_archive_commit(&key, receipt)
                .map_err(operation)?;
        }
    }
    let statuses = journal
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .statuses();
    for (key, status) in statuses {
        if status.state != SubmissionState::Committed || status.post_commit_receipt.is_some() {
            continue;
        }
        let (submission, receipt, capture) = {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            (
                journal.submission(&key).cloned(),
                status.archive_receipt.clone(),
                status.finalize_capture.clone(),
            )
        };
        let submission =
            submission.ok_or_else(|| operation("committed entry payload is not retained"))?;
        validate_accepted_submission(owner_id, &key, &submission)?;
        let receipt = receipt.ok_or_else(|| operation("committed entry lacks Archive receipt"))?;
        let capture = capture.ok_or_else(|| operation("committed entry lacks finalize capture"))?;
        if receipt.node_id != key.submission_id.as_str()
            || receipt.parent_node_id != capture.parent_node_id
            || receipt.branch_id != capture.branch_id
            || receipt.principal_id != capture.verified_principal_id
            || receipt.committed_timestamp_ns != capture.committed_timestamp_ns
            || receipt.payload_hash != submission.payload_hash
            || receipt.attachment_identities != submission.attachments
        {
            return Err(operation(
                "committed Archive receipt differs from captured submission",
            ));
        }
        let turn = runtime
            .conversation_turns(key.conversation_id.as_str(), &receipt.node_id)
            .map_err(operation)?
            .into_iter()
            .find(|turn| turn.node_id == receipt.node_id)
            .ok_or_else(|| operation("committed Archive node is unavailable during recovery"))?;
        validate_durable_turn(runtime, &turn, &submission, &capture)?;
        validate_archive_branch(
            runtime,
            key.conversation_id.as_str(),
            &capture.branch_id,
            &receipt.node_id,
        )?;
        restore_session_after_commit(
            runtime,
            key.conversation_id.as_str(),
            &receipt.node_id,
            capture.parent_node_id.as_deref(),
        )?;
        runtime
            .schedule_session(key.conversation_id.as_str(), episode_policy, now_ns())
            .map_err(operation)?;
        let post = PostCommitReceipt {
            operation_id: format!(
                "{owner_id}/{}/{}",
                key.conversation_id.as_str(),
                key.submission_id.as_str()
            ),
            scheduler_receipt: format!(
                "archive-node:{}:branch:{}:leaf:{}",
                receipt.node_id, receipt.branch_id, receipt.node_id
            ),
        };
        journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .record_postcommit_receipt(&key, post)
            .map_err(operation)?;
    }
    Ok(())
}

pub(super) fn archive_receipt_if_present(
    runtime: &mut crate::InteractionRuntime,
    key: &ConversationSubmissionId,
    submission: &Submission,
    capture: &FinalizeCapture,
) -> Result<Option<ArchiveReceipt>, ReliquaryRuntimeHostError> {
    let stable_id = key.submission_id.as_str();
    if !runtime
        .cva()
        .archive()
        .has_node(key.conversation_id.as_str(), stable_id)
    {
        return Ok(None);
    }
    let turn = runtime
        .conversation_turns(key.conversation_id.as_str(), stable_id)
        .map_err(operation)?
        .into_iter()
        .find(|turn| turn.node_id == stable_id)
        .ok_or_else(|| operation("stable Archive node is not in its own ancestry"))?;
    validate_durable_turn(runtime, &turn, submission, capture)?;
    finalization::validate_publication_head(runtime, key, capture)?;
    restore_session_after_commit(
        runtime,
        key.conversation_id.as_str(),
        stable_id,
        capture.parent_node_id.as_deref(),
    )?;
    publish_branch(
        runtime,
        key.conversation_id.as_str(),
        &capture.branch_id,
        stable_id,
        capture.branch_label.is_none(),
    )?;
    validate_archive_branch(
        runtime,
        key.conversation_id.as_str(),
        &capture.branch_id,
        stable_id,
    )?;
    Ok(Some(ArchiveReceipt {
        node_id: stable_id.to_owned(),
        branch_id: capture.branch_id.clone(),
        parent_node_id: capture.parent_node_id.clone(),
        principal_id: capture.verified_principal_id.clone(),
        committed_timestamp_ns: capture.committed_timestamp_ns,
        payload_hash: submission.payload_hash.clone(),
        attachment_identities: submission.attachments.clone(),
    }))
}
