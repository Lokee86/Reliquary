use super::*;

impl ConversationQueue {
    pub(crate) fn enqueue(
        &mut self,
        submission: QueuedSubmission,
        expected_revision: u64,
    ) -> Result<EnqueueOutcome, QueueError> {
        self.ensure()?;
        validate_id(&submission.principal_id, "principal id")?;
        let content_hash: [u8; 32] = Sha256::digest(submission.content.as_bytes()).into();
        if content_hash != submission.payload_hash {
            return Err(QueueError::DuplicatePayloadConflict);
        }
        if let Some(entry) = self.projection.entries.get(&submission.id) {
            if entry.submission != submission {
                return Err(QueueError::DuplicatePayloadConflict);
            }
            return Ok(EnqueueOutcome::Idempotent(entry.status()));
        }
        self.check_revision(&submission.id.conversation_id, expected_revision)?;
        let id = submission.id.clone();
        self.append(Event::Enqueued { submission })?;
        Ok(EnqueueOutcome::Durable(
            self.status(&id).expect("new entry is projected"),
        ))
    }
    pub(crate) fn cancel(
        &mut self,
        id: &ConversationSubmissionId,
        expected_revision: u64,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Cancelled {
            return Ok(entry.status());
        }
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if !entry
            .host_operations
            .values()
            .all(|op| op.phase == DurableHostOperationPhase::Cancelled)
            || entry
                .generation
                .as_ref()
                .is_some_and(|generation| generation.phase != DurableGenerationPhase::Cancelled)
        {
            return Err(QueueError::InvalidState);
        }
        self.check_revision(&id.conversation_id, expected_revision)?;
        self.append(Event::Cancelled {
            id: id.clone(),
            expected_revision,
        })?;
        Ok(self.status(id).expect("cancelled entry is projected"))
    }
    pub(crate) fn claim_finalize(
        &mut self,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Finalizing {
            if entry.finalize_capture.as_ref() == Some(&capture) {
                return Ok(entry.status());
            }
            return Err(QueueError::ArchiveIdentityConflict);
        }
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if !entry
            .host_operations
            .values()
            .all(|op| op.phase == DurableHostOperationPhase::Completed)
            || entry
                .generation
                .as_ref()
                .is_some_and(|generation| generation.phase != DurableGenerationPhase::Completed)
        {
            return Err(QueueError::InvalidState);
        }
        validate_id(&capture.branch_id, "branch id")?;
        if let Some(parent) = &capture.parent_node_id {
            validate_id(parent, "parent node id")?;
        }
        if capture.verified_principal_id != entry.submission.principal_id {
            return Err(QueueError::ArchiveIdentityConflict);
        }
        self.check_revision(&id.conversation_id, expected_revision)?;
        self.append(Event::FinalizeWon {
            id: id.clone(),
            expected_revision,
            capture,
        })?;
        Ok(self.status(id).expect("finalizing entry is projected"))
    }
    /// Record an Archive receipt only from the private publication seam.
    #[allow(dead_code)] // no writable Archive owner seam exists in this rebuild yet
    pub(crate) fn record_archive_commit(
        &mut self,
        id: &ConversationSubmissionId,
        proof: ArchiveCommitProof,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Committed {
            if entry.archive_receipt.as_ref() == Some(&proof.0) {
                return Ok(entry.status());
            }
            return Err(QueueError::ArchiveIdentityConflict);
        }
        if entry.state != QueueState::Finalizing {
            return Err(QueueError::InvalidState);
        }
        validate_receipt(entry, &proof.0)?;
        self.append(Event::ArchiveCommitted {
            id: id.clone(),
            receipt: proof.0,
        })?;
        Ok(self.status(id).expect("committed entry is projected"))
    }
}
