//! Authoritative P3 snapshot; P4 owns registration, cursors and delivery.
use super::super::observation::{
    BranchObservation, ConversationObservationSnapshot, GenerationIdentity, GenerationObservation,
    GenerationStatus, HostOperationObservation, SubmissionObservation, SubmissionOrder,
};
use super::*;
use crate::multiplexing_journal::DurableGenerationPhase;

impl GatewayRuntime {
    /// Copy one durable conversation cut through the requesting instance's grant.
    /// No session cursor, subscriber, provider call or second writer participates.
    #[allow(dead_code)] // Internal P3/P4 convergence seam.
    pub(in super::super) fn conversation_observation_snapshot(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        conversation_id: &str,
    ) -> Result<ConversationObservationSnapshot, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let conversation_id = ConversationId::new(conversation_id.to_owned()).map_err(operation)?;
        let journal = self.conversation.journal(owner_id)?;
        let coordinator = self
            .conversation
            .coordinator(owner_id, conversation_id.as_str())?;
        let snapshot = self.with_rel(&token, |runtime| {
            coordinator.observe(&journal, |journal| {
                // Existing owner durability barrier; no new mutation/version.
                // Fail closed on sync errors rather than publish unsynchronized heads.
                runtime.cva().sync().map_err(operation)?;
                let (policy, policy_revision) =
                    runtime.cva().conversation_policy(conversation_id.as_str());
                let statuses: Vec<_> = journal
                    .statuses()
                    .into_iter()
                    .filter(|(id, _)| id.conversation_id == conversation_id)
                    .collect();
                let mut branches: Vec<_> = runtime
                    .cva()
                    .archive()
                    .branches()
                    .into_iter()
                    .filter(|branch| branch.conversation_id == conversation_id.as_str())
                    .map(|branch| {
                        let capture = statuses
                            .iter()
                            .filter_map(|(_, status)| status.finalize_capture.as_ref())
                            .find(|capture| {
                                capture.branch_id == branch.id && capture.branch_label.is_some()
                            });
                        BranchObservation {
                            branch_id: branch.id,
                            head_node_id: Some(branch.leaf_node_id),
                            fork_parent_node_id: capture
                                .and_then(|capture| capture.parent_node_id.clone()),
                            display_label: capture.and_then(|capture| capture.branch_label.clone()),
                        }
                    })
                    .collect();
                branches.sort_by(|a, b| a.branch_id.cmp(&b.branch_id));
                let mut submissions = Vec::new();
                let mut generations = Vec::new();
                let mut host_operations = Vec::new();
                for (id, status) in statuses {
                    submissions.push(SubmissionObservation {
                        branch_id: journal
                            .observation_branch_id(&id)
                            .ok_or_else(|| operation("observation submission disappeared"))?
                            .to_owned(),
                        id: id.clone(),
                        state: status.state,
                        order: SubmissionOrder {
                            verified_receive_ns: status.verified_receive_ns,
                            tie_key: status.tie_key,
                            admission_sequence: status.admission_sequence,
                        },
                        transition_revision: status.transition_revision,
                    });
                    if let Some(generation) = status.generation {
                        generations.push(GenerationObservation {
                            identity: GenerationIdentity {
                                operation_id: generation.operation_id,
                                generation_id: generation.generation_id,
                                branch_id: generation.branch_id,
                            },
                            status: generation_status(generation.phase),
                            transition_revision: generation.transition_revision,
                            durable_checkpoint_sequence: generation
                                .checkpoint
                                .map(|checkpoint| checkpoint.sequence),
                        });
                    }
                    for (capability, host) in status.host_operations {
                        host_operations.push(HostOperationObservation {
                            submission_id: id.clone(),
                            operation_id: host.stable_id,
                            capability,
                            phase: host.phase,
                            transition_revision: host.transition_revision,
                            cancel_requested: host.cancel_requested,
                        });
                    }
                }
                submissions.sort_by_key(|entry| entry.order);
                generations.sort_by(|a, b| {
                    (&a.identity.branch_id, &a.identity.operation_id)
                        .cmp(&(&b.identity.branch_id, &b.identity.operation_id))
                });
                host_operations.sort_by(|a, b| {
                    (&a.submission_id, &a.operation_id).cmp(&(&b.submission_id, &b.operation_id))
                });
                let selection = instance
                    .managed_conversation_state(owner_id)
                    .map_err(operation)?;
                let selected_branch_id =
                    if selection.conversation_id.as_deref() == Some(conversation_id.as_str()) {
                        selection.branch_id
                    } else {
                        None
                    };
                let selected_head_node_id = selected_branch_id
                    .as_ref()
                    .and_then(|id| branches.iter().find(|branch| &branch.branch_id == id))
                    .and_then(|branch| branch.head_node_id.clone());
                Ok(ConversationObservationSnapshot {
                    rel_id: journal.rel_id(),
                    mount_epoch: token.mount_epoch,
                    conversation_id: conversation_id.clone(),
                    selected_branch_id,
                    selected_head_node_id,
                    policy,
                    policy_revision,
                    branches,
                    submissions,
                    generations,
                    host_operations,
                    durable_journal_sequence: journal.sequence(),
                })
            })
        })?;
        // Revalidate receipt delivery after releasing all semantic/journal locks.
        with_admission(self, &token, || Ok(snapshot))
    }
}

fn generation_status(phase: DurableGenerationPhase) -> GenerationStatus {
    match phase {
        DurableGenerationPhase::Pending => GenerationStatus::Pending,
        DurableGenerationPhase::Running => GenerationStatus::Running,
        DurableGenerationPhase::CancellationRequested => GenerationStatus::CancellationRequested,
        DurableGenerationPhase::Interrupted => GenerationStatus::Interrupted,
        DurableGenerationPhase::Unknown => GenerationStatus::Unknown,
        DurableGenerationPhase::Completed => GenerationStatus::Completed,
        DurableGenerationPhase::Cancelled => GenerationStatus::Cancelled,
    }
}
