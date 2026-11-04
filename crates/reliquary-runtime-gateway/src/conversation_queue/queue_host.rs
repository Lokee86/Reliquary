use super::*;

impl ConversationQueue {
    /// Persist one stable operation intent. `true` means a new intent was
    /// appended; `false` means one already exists and MUST NOT be redispatched.
    #[allow(dead_code)] // the host dispatcher is deliberately not implemented here
    pub(crate) fn begin_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
    ) -> Result<bool, QueueError> {
        validate_id(&stable_id, "host operation id")?;
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if let Some(previous) = entry.host_operations.get(&capability) {
            if previous.stable_id != stable_id || previous.payload_digest != payload_digest {
                return Err(QueueError::HostOperationConflict);
            }
            return Ok(false);
        }
        self.set_host_operation_phase(
            id,
            capability,
            stable_id,
            payload_digest,
            DurableHostOperationPhase::HandoffIntent,
        )?;
        Ok(true)
    }

    #[allow(dead_code)] // only a real durable host adapter may provide this receipt
    pub(crate) fn record_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        receipt: &HandoffReceipt,
    ) -> Result<SubmissionStatus, QueueError> {
        if !receipt.durably_accepted {
            return Err(QueueError::HostOperationConflict);
        }
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        if current.stable_id != receipt.stable_id
            || current.payload_digest != receipt.payload_digest
        {
            return Err(QueueError::HostOperationConflict);
        }
        if current.cancel_requested
            || matches!(
                current.phase,
                DurableHostOperationPhase::Cancelled
                    | DurableHostOperationPhase::Completed
                    | DurableHostOperationPhase::Pending
                    | DurableHostOperationPhase::Running
                    | DurableHostOperationPhase::RunningCancellationRequested
            )
        {
            return Ok(self.status(id).expect("operation belongs to queue entry"));
        }
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            DurableHostOperationPhase::Accepted,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // wired by startup/reconnect reconciliation in the host runtime
    pub(crate) fn record_host_status(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        status: HostOperationStatus,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        let phase = match status {
            HostOperationStatus::Pending if current.cancel_requested => {
                DurableHostOperationPhase::CancelIntent
            }
            HostOperationStatus::Pending => DurableHostOperationPhase::Pending,
            HostOperationStatus::Running if current.cancel_requested => {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostOperationStatus::Running => DurableHostOperationPhase::Running,
            HostOperationStatus::Completed => DurableHostOperationPhase::Completed,
            HostOperationStatus::Cancelled => DurableHostOperationPhase::Cancelled,
            HostOperationStatus::Unknown => DurableHostOperationPhase::Held,
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // no external host cancellation is dispatched by this crate
    pub(crate) fn begin_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            DurableHostOperationPhase::CancelIntent,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // only a conforming host adapter may provide this outcome
    pub(crate) fn record_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        outcome: HostCancelOutcome,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        let phase = match outcome {
            HostCancelOutcome::CancelledBeforeStart
            | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Cancelled) => {
                DurableHostOperationPhase::Cancelled
            }
            HostCancelOutcome::RunningCancellationRequested => {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Completed) => {
                DurableHostOperationPhase::Completed
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Pending)
                if current.cancel_requested =>
            {
                DurableHostOperationPhase::CancelIntent
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Pending) => {
                DurableHostOperationPhase::Pending
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Running)
                if current.cancel_requested =>
            {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Running) => {
                DurableHostOperationPhase::Running
            }
            HostCancelOutcome::Unknown
            | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Unknown) => {
                DurableHostOperationPhase::Held
            }
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // transition primitive for the future host runtime seam
    fn set_host_operation_phase(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
        phase: DurableHostOperationPhase,
    ) -> Result<(), QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        let previous = entry.host_operations.get(&capability);
        let (revision, cancel_requested, last_known_phase) = match previous {
            Some(old) => {
                if old.stable_id != stable_id || old.payload_digest != payload_digest {
                    return Err(QueueError::HostOperationConflict);
                }
                if old.phase == phase {
                    return Ok(());
                }
                if !host_phase_transition_allowed(&old.phase, &phase) {
                    if old.phase != DurableHostOperationPhase::Held
                        && matches!(
                            phase,
                            DurableHostOperationPhase::Pending | DurableHostOperationPhase::Running
                        )
                        && host_phase_transition_allowed(&phase, &old.phase)
                    {
                        return Ok(());
                    }
                    return Err(QueueError::InvalidState);
                }
                let revision = old
                    .transition_revision
                    .checked_add(1)
                    .ok_or(QueueError::SequenceOverflow)?;
                let sticky_cancel = old.cancel_requested
                    || matches!(
                        phase,
                        DurableHostOperationPhase::CancelIntent
                            | DurableHostOperationPhase::RunningCancellationRequested
                    );
                let last_known = if phase == DurableHostOperationPhase::Held {
                    Some(
                        old.last_known_phase
                            .clone()
                            .unwrap_or_else(|| old.phase.clone()),
                    )
                } else {
                    Some(phase.clone())
                };
                (revision, sticky_cancel, last_known)
            }
            None => {
                if phase != DurableHostOperationPhase::HandoffIntent {
                    return Err(QueueError::InvalidState);
                }
                (1, false, None)
            }
        };
        self.append(Event::HostOperationUpdated {
            id: id.clone(),
            capability,
            operation: DurableHostOperation {
                stable_id,
                payload_digest,
                phase,
                transition_revision: revision,
                cancel_requested,
                last_known_phase,
            },
        })
    }
}
