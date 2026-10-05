//! Authorized broad source reader; no nested journal/runtime locks.
use super::*;

impl GatewayRuntime {
    pub(in super::super) fn read_rel_snapshot_state(
        &self,
        token: &OwnerAccessToken,
    ) -> Result<RelSnapshotState, ReliquaryRuntimeHostError> {
        self.read_rel_snapshot_state_with_hook(token, || {})
    }

    pub(super) fn read_rel_snapshot_state_with_hook(
        &self,
        token: &OwnerAccessToken,
        mut after_semantic_read: impl FnMut(),
    ) -> Result<RelSnapshotState, ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(&token.owner_id)?;
        // Journal cut brackets the semantic read. No journal/runtime lock nesting.
        // A racing legacy journal writer causes retry rather than a torn snapshot.
        for _ in 0..8 {
            let (sequence, mut submissions, host_work) = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let statuses = journal.statuses();
                if statuses.len() > MAX_SNAPSHOT_ITEMS {
                    return Err(operation("REL snapshot exceeds its item bound"));
                }
                let submissions = statuses
                    .into_iter()
                    .map(|(id, status)| {
                        let branch_id = status
                            .finalize_capture
                            .as_ref()
                            .map(|capture| capture.branch_id.clone())
                            .or_else(|| {
                                journal.submission(&id).and_then(|submission| {
                                    submission
                                        .request_metadata
                                        .get("branch_id")
                                        .and_then(|id| id.as_str())
                                        .map(str::to_owned)
                                })
                            });
                        RelSubmissionSummary {
                            id,
                            branch_id,
                            state: status.state,
                            order: SubmissionOrder {
                                verified_receive_ns: status.verified_receive_ns,
                                tie_key: status.tie_key,
                                admission_sequence: status.admission_sequence,
                            },
                            transition_revision: status.transition_revision,
                        }
                    })
                    .collect::<Vec<_>>();
                let host_work = journal
                    .host_operations()
                    .into_iter()
                    .map(
                        |(submission_id, capability, host)| HostOperationObservation {
                            submission_id,
                            operation_id: host.stable_id,
                            capability,
                            phase: host.phase,
                            transition_revision: host.transition_revision,
                            cancel_requested: host.cancel_requested,
                        },
                    )
                    .collect::<Vec<_>>();
                if host_work.len() > MAX_SNAPSHOT_ITEMS {
                    return Err(operation("REL snapshot exceeds its item bound"));
                }
                (journal.sequence(), submissions, host_work)
            };
            submissions.sort_by(|a, b| {
                a.id.conversation_id
                    .cmp(&b.id.conversation_id)
                    .then(a.order.cmp(&b.order))
                    .then(a.id.submission_id.cmp(&b.id.submission_id))
            });
            let mut state = self.with_rel(token, |runtime| {
                let cva = runtime.cva();
                let mut conversations = cva.conversation_summaries();
                conversations.sort_by(|a, b| a.conversation_id.cmp(&b.conversation_id));
                let mut branches = cva.branches();
                branches.sort_by(|a, b| {
                    a.conversation_id
                        .cmp(&b.conversation_id)
                        .then(a.id.cmp(&b.id))
                });
                if conversations.len() > MAX_SNAPSHOT_ITEMS || branches.len() > MAX_SNAPSHOT_ITEMS {
                    return Err(operation("REL snapshot exceeds its item bound"));
                }
                let project = cva
                    .latest_project_revision_correlation()
                    .map(|correlation| RelProjectStatus {
                        correlation_sequence: correlation.sequence,
                        repository_kind: correlation.project_revision.repository.kind,
                        management: correlation.repository_management,
                    });
                Ok(RelSnapshotState {
                    conversations,
                    branches,
                    submissions,
                    host_work,
                    processing: cva.insomnia_stats(),
                    processing_enabled: true,
                    project,
                    lifecycle: OwnerAdmission::Open,
                    watermarks: source_watermarks(cva),
                    conversation_observation: None,
                })
            })?;
            let enabled = {
                let owner = {
                    let rels = self
                        .rel_executions
                        .read()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    Arc::clone(
                        &rels
                            .get(&token.owner_id)
                            .ok_or_else(|| operation("REL is no longer mounted"))?
                            .owner,
                    )
                };
                let execution = owner
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                execution.insomnia_enabled.load(Ordering::SeqCst)
            };
            state.processing_enabled = enabled;
            after_semantic_read();
            if journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .sequence()
                == sequence
            {
                state.watermarks.conversation_journal_sequence = Some(sequence);
                return Ok(state);
            }
        }
        Err(operation(
            "REL journal changed repeatedly during snapshot; retry subscription",
        ))
    }
}

pub(in super::super) fn source_watermarks(cva: &crate::Cva) -> DurableWatermarks {
    DurableWatermarks {
        global_version: Some(cva.latest_global_version()),
        archive_version: Some(cva.archive_version()),
        memory_version: Some(cva.memory_version()),
        entity_version: Some(cva.entity_version()),
        graph_version: Some(cva.graph_version()),
        ego_mutation_version: Some(cva.ego_version()),
        vector_version: Some(cva.vector_version()),
        project_correlation_sequence: cva
            .latest_project_revision_correlation()
            .map(|entry| entry.sequence),
        conversation_journal_sequence: None,
    }
}
