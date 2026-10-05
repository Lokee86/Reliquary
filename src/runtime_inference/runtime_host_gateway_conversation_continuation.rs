use super::*;

/// Derived durable debt plus the live attempt, keyed by stable Archive branch ID.
/// Journal records remain authoritative; a returned Pending host operation is
/// still debt, even though no thread is currently driving it.
#[derive(Default)]
struct BranchContinuation {
    executing: Option<ConversationSubmissionId>,
    durable: Option<ConversationSubmissionId>,
}
#[derive(Default)]
struct ConversationState {
    branches: BTreeMap<String, BranchContinuation>,
}
pub(super) struct ConversationCoordinator {
    state: Mutex<ConversationState>,
}
impl ConversationCoordinator {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(ConversationState::default()),
        }
    }

    /// Selection and branch reservation share one lock acquisition. The journal
    /// supplies order and recovery work; this state holds no duplicate queue.
    pub(super) fn acquire_next(
        self: &Arc<Self>,
        journal: &Arc<Mutex<RelJournal>>,
        conversation_id: &ConversationId,
    ) -> Result<Option<(ConversationSubmissionId, ProcessingGuard)>, ReliquaryRuntimeHostError>
    {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let journal = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        refresh_debt(&mut state, &journal, conversation_id)?;
        let mut entries = journal.statuses();
        entries.retain(|(key, status)| {
            &key.conversation_id == conversation_id
                && (status.state == SubmissionState::Pending
                    || status.state == SubmissionState::Finalizing
                    || (status.state == SubmissionState::Committed
                        && status.post_commit_receipt.is_none()))
        });
        entries.sort_by_key(|(_, status)| {
            (
                status.verified_receive_ns,
                status.tie_key,
                status.admission_sequence,
            )
        });
        for (key, status) in entries {
            // Host work already dispatched is a continuation, not a fresh queue
            // candidate. Explicit reconciliation must not starve an idle branch.
            if status.state == SubmissionState::Pending
                && (status
                    .generation
                    .as_ref()
                    .is_some_and(|generation| !generation.phase.terminal())
                    || journal
                        .host_operation(&key, RuntimeCapability::ConversationExecution)
                        .is_some_and(|host| {
                            !matches!(
                                host.phase,
                                DurableHostOperationPhase::Completed
                                    | DurableHostOperationPhase::Cancelled
                            )
                        }))
            {
                continue;
            }
            let branch_id =
                match status.finalize_capture {
                    Some(capture) => capture.branch_id,
                    None => metadata_branch(journal.submission(&key).ok_or_else(|| {
                        operation("unfinished submission payload is not retained")
                    })?),
                };
            let branch = state.branches.entry(branch_id.clone()).or_default();
            if branch.executing.is_some()
                || branch.durable.as_ref().is_some_and(|owner| owner != &key)
            {
                continue;
            }
            branch.executing = Some(key.clone());
            return Ok(Some((
                key,
                ProcessingGuard {
                    coordinator: Arc::clone(self),
                    branch_id,
                },
            )));
        }
        Ok(None)
    }

    pub(super) fn acquire(
        self: &Arc<Self>,
        journal: &Arc<Mutex<RelJournal>>,
        key: &ConversationSubmissionId,
        branch_id: &str,
    ) -> Result<ProcessingGuard, ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let journal = journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        refresh_debt(&mut state, &journal, &key.conversation_id)?;
        let branch = state.branches.entry(branch_id.to_owned()).or_default();
        if branch.executing.is_some() {
            return Err(operation("branch already has an active finalizer"));
        }
        if branch.durable.as_ref().is_some_and(|owner| owner != key) {
            return Err(operation("branch is fenced by an unresolved continuation"));
        }
        // Fresh exact-entry acquisition exists only in test fixtures. Durable
        // continuation resumption keeps its reservation even if newer ties arrive.
        if branch.durable.is_none()
            && journal
                .status(key)
                .is_some_and(|status| status.state == SubmissionState::Pending)
        {
            let next = journal
                .pending_order(&key.conversation_id)
                .into_iter()
                .find(|candidate| {
                    journal
                        .submission(candidate)
                        .is_some_and(|submission| metadata_branch(submission) == branch_id)
                });
            if next.as_ref() != Some(key) {
                return Err(operation("submission is not the next queued branch turn"));
            }
        }
        branch.executing = Some(key.clone());
        Ok(ProcessingGuard {
            coordinator: Arc::clone(self),
            branch_id: branch_id.to_owned(),
        })
    }
}

fn refresh_debt(
    state: &mut ConversationState,
    journal: &RelJournal,
    conversation_id: &ConversationId,
) -> Result<(), ReliquaryRuntimeHostError> {
    let mut debt = BTreeMap::new();
    for (key, status) in journal.statuses() {
        if &key.conversation_id != conversation_id {
            continue;
        }
        let generation_active = status
            .generation
            .as_ref()
            .is_some_and(|generation| !generation.phase.terminal());
        let unresolved = generation_active
            || match status.state {
                SubmissionState::Finalizing => true,
                SubmissionState::Committed => status.post_commit_receipt.is_none(),
                SubmissionState::Pending => journal
                    .host_operation(&key, RuntimeCapability::ConversationExecution)
                    .is_some(),
                SubmissionState::Cancelled => false,
            };
        if !unresolved {
            continue;
        }
        let branch_id =
            if let Some(generation) = status.generation.as_ref().filter(|_| generation_active) {
                generation.branch_id.clone()
            } else {
                match status.finalize_capture {
                    Some(capture) => capture.branch_id,
                    None => metadata_branch(journal.submission(&key).ok_or_else(|| {
                        operation("unresolved continuation payload is not retained")
                    })?),
                }
            };
        if debt.insert(branch_id, key).is_some() {
            return Err(operation(
                "multiple durable continuations own the same branch",
            ));
        }
    }
    for branch in state.branches.values_mut() {
        branch.durable = None;
    }
    for (branch_id, key) in debt {
        state.branches.entry(branch_id).or_default().durable = Some(key);
    }
    Ok(())
}

pub(super) struct ProcessingGuard {
    coordinator: Arc<ConversationCoordinator>,
    branch_id: String,
}
impl Drop for ProcessingGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.coordinator.state.lock() {
            if let Some(branch) = state.branches.get_mut(&self.branch_id) {
                branch.executing = None;
            }
        }
    }
}

impl ConversationRegistry {
    /// Reconstruct idle branch inventory from Archive and unfinished work from
    /// the journal before owner workers start. No viewer state participates.
    pub(super) fn rebuild_continuations(
        &self,
        owner_id: &str,
        branches: Vec<Branch>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let journal = self.journal(owner_id)?;
        let conversations = {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            journal
                .statuses()
                .into_iter()
                .map(|(key, _)| key.conversation_id.as_str().to_owned())
                .chain(branches.iter().map(|branch| branch.conversation_id.clone()))
                .collect::<BTreeSet<_>>()
        };
        for conversation_id in conversations {
            let coordinator = self.coordinator(owner_id, &conversation_id)?;
            let mut state = coordinator
                .state
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            for branch in branches
                .iter()
                .filter(|branch| branch.conversation_id == conversation_id)
            {
                state.branches.entry(branch.id.clone()).or_default();
            }
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            refresh_debt(
                &mut state,
                &journal,
                &ConversationId::new(conversation_id).map_err(operation)?,
            )?;
        }
        Ok(())
    }
}
