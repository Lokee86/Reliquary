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
            let sequence = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .sequence();
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
                    submissions: Vec::new(),
                    host_work: Vec::new(),
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
            let observation = self.read_conversation_distribution_source(
                &token.owner_id,
                token.mount_epoch,
                None,
            )?;
            state.submissions = observation
                .conversations
                .iter()
                .flat_map(|conversation| conversation.submissions.iter())
                .map(|submission| RelSubmissionSummary {
                    id: submission.id.clone(),
                    branch_id: Some(submission.branch_id.clone()),
                    state: submission.state,
                    order: submission.order,
                    transition_revision: submission.transition_revision,
                })
                .collect();
            state.host_work = observation
                .conversations
                .iter()
                .flat_map(|conversation| conversation.host_operations.iter().cloned())
                .collect();
            state.watermarks = observation.durable_watermarks.clone();
            state.conversation_observation = Some(observation);
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
