use super::*;

impl Instance {
    /// Admit a provisional user submission using only this Instance's verified
    /// principal and exact REL grant. Queue contents never enter Archive here.
    pub fn submit_conversation(
        &self,
        queue: &mut ConversationQueue,
        conversation_id: impl Into<String>,
        submission_id: SubmissionId,
        content: impl Into<String>,
        expected_revision: u64,
    ) -> Result<EnqueueOutcome, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let id = ConversationSubmissionId::new(conversation_id, submission_id)?;
        let submission = QueuedSubmission::new(id, self.principal.to_string(), content)?;
        Ok(queue.enqueue(submission, expected_revision)?)
    }

    /// Win the finalize race, sync the accepted turn and branch through the
    /// mounted REL Archive, then record the receipt in the durable queue.
    pub fn finalize_conversation_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    ) -> Result<SubmissionStatus, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let submitter = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if submitter != self.principal.as_ref() {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: self.principal.to_string(),
                owner: queue.owner(),
            });
        }
        if let Some(status) = queue.status(id)
            && status.state == QueueState::Committed
        {
            if status.finalize_capture.as_ref() == Some(&capture) {
                return Ok(status);
            }
            return Err(conversation_queue::QueueError::ArchiveIdentityConflict.into());
        }
        let submission = queue
            .submission(id)
            .cloned()
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&queue.owner()) {
                None => return Err(GatewayError::UnknownOwner(queue.owner())),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(_) => return Err(GatewayError::WriterUnavailable),
            }
        };
        let publication = reliquary_application::NativeConversationCommit {
            node_id: submission.id.submission_id.as_str().to_owned(),
            conversation_id: submission.id.conversation_id.clone(),
            parent_node_id: capture.parent_node_id.clone(),
            branch_id: capture.branch_id.clone(),
            principal_id: submission.principal_id.clone(),
            committed_timestamp_ns: capture.committed_timestamp_ns,
            content: submission.content,
        };
        let (finalizing, node) = {
            let mut writer = mounted
                .writer
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            self.ensure_credential_active()?;
            reliquary_application::validate_native_conversation_commit(&writer, &publication)?;
            let finalizing = queue.claim_finalize(id, expected_revision, capture.clone())?;
            let node =
                reliquary_application::commit_native_conversation_turn(&mut writer, publication)?;
            (finalizing, node)
        };

        let receipt = ArchiveReceipt {
            node_id: node.node_id,
            branch_id: capture.branch_id,
            parent_node_id: node.parent_node_id,
            principal_id: node
                .principal_id
                .ok_or(conversation_queue::QueueError::ArchiveIdentityConflict)?,
            committed_timestamp_ns: node.committed_timestamp_ns,
            payload_hash: submission.payload_hash,
        };
        if finalizing.state != QueueState::Finalizing {
            return Err(conversation_queue::QueueError::InvalidState.into());
        }
        queue.record_archive_commit(id, ArchiveCommitProof::from_archive(receipt))?;
        Ok(queue.status(id).expect("committed entry remains projected"))
    }

    /// Open the queue for a mounted REL only after checking this Instance's grant.
    pub fn open_conversation_queue(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, GatewayError> {
        self.check_queue_mount(owner)?;
        Ok(ConversationQueue::open(state_dir, owner)?)
    }

    pub fn open_conversation_queue_read(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, GatewayError> {
        self.check_queue_mount(owner)?;
        let queue = ConversationQueue::open_read(state_dir, owner)?;
        self.ensure_credential_active()?;
        Ok(queue)
    }

    fn check_queue_mount(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.authorize_queue_owner(owner)?;
        {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                Some(mounted) if mounted.is_rel() => {}
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
                None => return Err(GatewayError::UnknownOwner(owner)),
            }
        }
        self.ensure_credential_active()
    }

    /// Read one operation status through the same owner authorization check used
    /// for submission and cancellation.
    pub fn conversation_submission_status(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<Option<SubmissionStatus>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.status(id))
    }

    pub fn host_operation_status(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Result<Option<DurableHostOperation>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.host_operation(id, capability))
    }

    /// Read the latest full text prefix only after checking the current REL grant.
    pub fn generation_checkpoint_snapshot(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<Option<DurableGenerationCheckpoint>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.generation_checkpoint(id))
    }

    /// Replay bounded, already-synced full-prefix checkpoints for this REL.
    pub fn replay_generation_checkpoints(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
        after_sequence: u64,
        max_items: usize,
    ) -> Result<GenerationReplay, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.replay_generation_checkpoints(id, after_sequence, max_items)?)
    }

    /// Cancel only a pending submission originally admitted under this principal.
    pub fn cancel_own_pending_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
    ) -> Result<SubmissionStatus, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let submitter = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if submitter != self.principal.as_ref() {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: self.principal.to_string(),
                owner: queue.owner(),
            });
        }
        Ok(queue.cancel(id, expected_revision)?)
    }

    pub(super) fn authorize_queue_owner(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        Ok(())
    }
}
