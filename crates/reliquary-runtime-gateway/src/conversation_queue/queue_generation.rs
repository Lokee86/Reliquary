use super::*;

impl ConversationQueue {
    #[allow(dead_code)] // trusted generation driver only; no public provider dispatch exists
    pub(crate) fn begin_generation(
        &mut self,
        id: &ConversationSubmissionId,
        operation_id: String,
        generation_id: String,
        branch_id: String,
        parent_node_id: String,
        started_timestamp_ns: i64,
    ) -> Result<SubmissionStatus, QueueError> {
        for (value, kind) in [
            (&operation_id, "generation operation id"),
            (&generation_id, "generation id"),
            (&branch_id, "branch id"),
            (&parent_node_id, "parent node id"),
        ] {
            validate_id(value, kind)?;
        }
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if let Some(previous) = &entry.generation {
            if previous.operation_id == operation_id
                && previous.generation_id == generation_id
                && previous.branch_id == branch_id
                && previous.parent_node_id == parent_node_id
                && previous.started_timestamp_ns == started_timestamp_ns
            {
                return Ok(entry.status());
            }
            return Err(QueueError::HostOperationConflict);
        }
        let generation = DurableGeneration {
            operation_id,
            generation_id,
            branch_id,
            parent_node_id,
            started_timestamp_ns,
            phase: DurableGenerationPhase::Running,
            transition_revision: 1,
            stream_cursor: 0,
            checkpoint: None,
        };
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }

    #[allow(dead_code)] // trusted generation driver only; content becomes visible after sync
    pub(crate) fn record_generation_checkpoint(
        &mut self,
        id: &ConversationSubmissionId,
        sequence: u64,
        content_prefix: String,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        if content_prefix.len() > MAX_CHECKPOINT_TEXT_BYTES {
            return Err(QueueError::JournalFull);
        }
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        let current = entry.generation.as_ref().ok_or(QueueError::InvalidState)?;
        if current.phase != DurableGenerationPhase::Running {
            return Err(QueueError::InvalidState);
        }
        let bytes = u64::try_from(content_prefix.len()).map_err(|_| QueueError::JournalFull)?;
        let digest: [u8; 32] = Sha256::digest(content_prefix.as_bytes()).into();
        if let Some(previous) = &current.checkpoint {
            if sequence == previous.sequence {
                if previous.utf8_bytes == bytes
                    && previous.sha256 == digest
                    && previous.content_prefix == content_prefix
                {
                    return Ok(entry.status());
                }
                return Err(QueueError::DuplicatePayloadConflict);
            }
            if sequence
                != previous
                    .sequence
                    .checked_add(1)
                    .ok_or(QueueError::SequenceOverflow)?
            {
                return Err(QueueError::InvalidReplayCursor);
            }
            if !content_prefix.starts_with(&previous.content_prefix) {
                return Err(QueueError::DuplicatePayloadConflict);
            }
        } else if sequence != 1 {
            return Err(QueueError::InvalidReplayCursor);
        }
        let mut next = current.clone();
        next.transition_revision = next
            .transition_revision
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        next.stream_cursor = next
            .stream_cursor
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        let delivery_cursor = current
            .checkpoint
            .as_ref()
            .map_or(Some(1), |checkpoint| {
                checkpoint.delivery_cursor.checked_add(1)
            })
            .ok_or(QueueError::SequenceOverflow)?;
        next.checkpoint = Some(DurableGenerationCheckpoint {
            sequence,
            delivery_cursor,
            utf8_bytes: bytes,
            sha256: digest,
            content_prefix,
        });
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation: next,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }

    #[allow(dead_code)] // lifecycle reconciliation remains an internal trusted seam
    pub(crate) fn set_generation_phase(
        &mut self,
        id: &ConversationSubmissionId,
        phase: DurableGenerationPhase,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        let mut generation = entry.generation.clone().ok_or(QueueError::InvalidState)?;
        if generation.phase == phase {
            return Ok(entry.status());
        }
        if !generation_phase_transition_allowed(generation.phase, phase) {
            return Err(QueueError::InvalidState);
        }
        generation.phase = phase;
        generation.transition_revision = generation
            .transition_revision
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        generation.stream_cursor = generation
            .stream_cursor
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }
}
