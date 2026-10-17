//! Bounded conversation diffing and safe delta construction for P4.7.
use super::super::events::{RelEventClass, RelInvalidation};
use super::super::observation::{
    BranchObservation, CheckpointContentIdentity, ConversationDurableDelta, ConversationTransition,
    GenerationCheckpointObservation, GenerationObservation, HostOperationObservation,
    RelConversationSummary, RelObservationSummary, SubmissionObservation,
};
use super::super::rel_journal_owner_id;
use super::{GenerationKey, ReliquaryRuntimeHostError, operation};
use crate::multiplexing_journal::{ConversationId, ConversationSubmissionId};
use std::collections::BTreeMap;

pub(super) struct QueuedBroadEvent {
    pub(super) class: RelEventClass,
    pub(super) invalidation: RelInvalidation,
}

pub(super) fn checkpoint_position_map<'a>(
    owner_id: &str,
    epoch: u64,
    positions: &'a [GenerationCheckpointObservation],
) -> Result<BTreeMap<GenerationKey, &'a GenerationCheckpointObservation>, ReliquaryRuntimeHostError>
{
    let rel_id = rel_journal_owner_id(owner_id)?;
    if positions.len() > 16_384 {
        return Err(operation("checkpoint position source exceeds its bound"));
    }
    let mut result = BTreeMap::new();
    for position in positions {
        if position.rel_id != rel_id
            || position.mount_epoch != epoch
            || position.durable_checkpoint_sequence == 0
            || !matches!(&position.content, CheckpointContentIdentity::Prefix { .. })
        {
            return Err(operation(
                "checkpoint position is outside its REL epoch or shape",
            ));
        }
        let key = GenerationKey {
            conversation_id: position.conversation_id.clone(),
            operation_id: position.generation.operation_id.clone(),
            generation_id: position.generation.generation_id.clone(),
            branch_id: position.generation.branch_id.clone(),
        };
        if result.insert(key, position).is_some() {
            return Err(operation("duplicate durable checkpoint position"));
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ReceiptFactKind {
    FinalizeWon,
    ArchiveCommitted,
    PostCommitRecorded,
}

fn receipt_fact_key(
    fact: &ConversationDurableDelta,
) -> Result<(ConversationId, ConversationSubmissionId, ReceiptFactKind), ReliquaryRuntimeHostError>
{
    let (submission_id, kind) = match &fact.transition {
        ConversationTransition::FinalizeWon { submission_id, .. } => {
            (submission_id, ReceiptFactKind::FinalizeWon)
        }
        ConversationTransition::ArchiveCommitted { submission_id, .. } => {
            (submission_id, ReceiptFactKind::ArchiveCommitted)
        }
        ConversationTransition::PostCommitRecorded { submission_id, .. } => {
            (submission_id, ReceiptFactKind::PostCommitRecorded)
        }
        _ => {
            return Err(operation(
                "receipt source returned a non-receipt conversation transition",
            ));
        }
    };
    Ok((fact.conversation_id.clone(), submission_id.clone(), kind))
}

fn receipt_fact_map<'a>(
    facts: &'a [ConversationDurableDelta],
) -> Result<
    BTreeMap<
        (ConversationId, ConversationSubmissionId, ReceiptFactKind),
        &'a ConversationTransition,
    >,
    ReliquaryRuntimeHostError,
> {
    let mut result = BTreeMap::new();
    for fact in facts {
        let key = receipt_fact_key(fact)?;
        if result.insert(key, &fact.transition).is_some() {
            return Err(operation("duplicate durable conversation receipt fact"));
        }
    }
    Ok(result)
}

pub(super) fn receipt_facts_changed(
    before: &[ConversationDurableDelta],
    after: &[ConversationDurableDelta],
) -> bool {
    match (receipt_fact_map(before), receipt_fact_map(after)) {
        (Ok(before), Ok(after)) => before != after,
        _ => true,
    }
}

pub(super) fn validate_receipt_facts(
    owner_id: &str,
    epoch: u64,
    facts: &[ConversationDurableDelta],
) -> Result<(), ReliquaryRuntimeHostError> {
    let rel_id = rel_journal_owner_id(owner_id)?;
    if facts.len() > 16_384
        || facts
            .iter()
            .any(|fact| fact.rel_id != rel_id || fact.mount_epoch != epoch)
    {
        return Err(operation(
            "conversation receipt facts exceed their REL epoch/bound",
        ));
    }
    let _ = receipt_fact_map(facts)?;
    Ok(())
}

pub(super) fn validate_distribution_identity(
    owner_id: &str,
    epoch: u64,
    summary: &RelObservationSummary,
) -> Result<(), ReliquaryRuntimeHostError> {
    let rel_id = rel_journal_owner_id(owner_id)?;
    if summary.rel_id != rel_id || summary.mount_epoch != epoch {
        return Err(operation(
            "conversation distribution source belongs to another REL epoch",
        ));
    }
    if summary.conversations.len() > 16_384
        || summary.conversations.iter().any(|conversation| {
            conversation.branches.len() > 16_384
                || conversation.submissions.len() > 16_384
                || conversation.generations.len() > 16_384
                || conversation.host_operations.len() > 16_384
        })
    {
        return Err(operation(
            "conversation distribution source exceeds its bound",
        ));
    }
    Ok(())
}

fn conversation_map(
    summary: &RelObservationSummary,
) -> BTreeMap<ConversationId, &RelConversationSummary> {
    summary
        .conversations
        .iter()
        .map(|conversation| (conversation.conversation_id.clone(), conversation))
        .collect()
}

pub(super) fn derive_broad_events(
    before: &RelObservationSummary,
    after: &RelObservationSummary,
    before_receipts: &[ConversationDurableDelta],
    after_receipts: &[ConversationDurableDelta],
) -> Result<Vec<QueuedBroadEvent>, ReliquaryRuntimeHostError> {
    let before_map = conversation_map(before);
    let after_map = conversation_map(after);
    let before_receipt_map = receipt_fact_map(before_receipts)?;
    let after_receipt_map = receipt_fact_map(after_receipts)?;
    if before_receipt_map.iter().any(|(key, transition)| {
        after_receipt_map
            .get(key)
            .is_none_or(|next| *next != *transition)
    }) {
        return Err(operation(
            "durable conversation receipt disappeared; fresh snapshot required",
        ));
    }
    if before_map.keys().any(|id| !after_map.contains_key(id)) {
        return Err(operation(
            "durable conversation disappeared; fresh snapshot required",
        ));
    }
    let rel_id = after.rel_id;
    let epoch = after.mount_epoch;
    let watermarks = after.durable_watermarks.clone();
    let mut result = Vec::new();
    for (conversation_id, current) in after_map {
        let prior = before_map.get(&conversation_id).copied();
        let push = |result: &mut Vec<QueuedBroadEvent>, class, transition| {
            let delta = ConversationDurableDelta {
                rel_id,
                mount_epoch: epoch,
                conversation_id: conversation_id.clone(),
                durable_watermarks: watermarks.clone(),
                transition,
            };
            result.push(QueuedBroadEvent {
                class,
                invalidation: RelInvalidation::ConversationDelta(delta),
            });
        };

        if prior.map(|old| (&old.policy, old.policy_revision))
            != Some((&current.policy, current.policy_revision))
        {
            push(
                &mut result,
                RelEventClass::ArchiveChanged,
                ConversationTransition::PolicyChanged {
                    policy: current.policy,
                    revision: current.policy_revision,
                },
            );
        }

        let old_submissions = prior.map(submission_map).unwrap_or_default();
        let current_submissions = submission_map(current);
        if old_submissions
            .keys()
            .any(|id| !current_submissions.contains_key(id))
        {
            return Err(operation(
                "durable conversation submission disappeared; fresh snapshot required",
            ));
        }
        for submission in &current.submissions {
            if old_submissions.get(&submission.id).copied() != Some(submission) {
                push(
                    &mut result,
                    RelEventClass::SubmissionQueueChanged,
                    ConversationTransition::SubmissionChanged(submission.clone()),
                );
            }
        }

        let old_branches = prior.map(branch_map).unwrap_or_default();
        let current_branches = branch_map(current);
        if old_branches
            .keys()
            .any(|id| !current_branches.contains_key(id))
        {
            return Err(operation(
                "durable Archive branch disappeared; fresh snapshot required",
            ));
        }
        for branch in &current.branches {
            match old_branches.get(&branch.branch_id).copied() {
                None if branch.head_node_id.is_some() => push(
                    &mut result,
                    RelEventClass::ArchiveChanged,
                    ConversationTransition::BranchCreated(branch.clone()),
                ),
                Some(old) if old != branch => {
                    let published_now = old.head_node_id.is_none() && branch.head_node_id.is_some();
                    if published_now {
                        push(
                            &mut result,
                            RelEventClass::ArchiveChanged,
                            ConversationTransition::BranchCreated(branch.clone()),
                        );
                    } else {
                        result.push(QueuedBroadEvent {
                            class: RelEventClass::ArchiveChanged,
                            invalidation: RelInvalidation::Conversation(conversation_id.clone()),
                        });
                    }
                }
                _ => {}
            }
        }

        let old_generations = prior
            .map(generation_map_for_conversation)
            .unwrap_or_default();
        for generation in &current.generations {
            let key = GenerationKey::new(&conversation_id, generation);
            if old_generations.get(&key).copied() != Some(generation) {
                push(
                    &mut result,
                    RelEventClass::ConversationOperationChanged,
                    ConversationTransition::GenerationChanged(generation.clone()),
                );
            }
        }

        let old_hosts = prior.map(host_operation_map).unwrap_or_default();
        let current_hosts = host_operation_map(current);
        if old_hosts.keys().any(|id| !current_hosts.contains_key(id)) {
            return Err(operation(
                "durable host operation disappeared; fresh snapshot required",
            ));
        }
        for host in &current.host_operations {
            let key = (host.submission_id.clone(), host.operation_id.clone());
            if old_hosts.get(&key).copied() != Some(host) {
                push(
                    &mut result,
                    RelEventClass::HostWorkChanged,
                    ConversationTransition::HostOperationChanged(host.clone()),
                );
            }
        }
    }

    for fact in after_receipts {
        let key = receipt_fact_key(fact)?;
        if before_receipt_map
            .get(&key)
            .is_none_or(|old| *old != &fact.transition)
        {
            let class = match &fact.transition {
                ConversationTransition::FinalizeWon { .. }
                | ConversationTransition::PostCommitRecorded { .. } => {
                    RelEventClass::SubmissionQueueChanged
                }
                ConversationTransition::ArchiveCommitted { .. } => RelEventClass::ArchiveChanged,
                _ => {
                    return Err(operation(
                        "receipt source returned a non-receipt conversation transition",
                    ));
                }
            };
            result.push(QueuedBroadEvent {
                class,
                invalidation: RelInvalidation::ConversationDelta(ConversationDurableDelta {
                    rel_id: after.rel_id,
                    mount_epoch: after.mount_epoch,
                    conversation_id: fact.conversation_id.clone(),
                    durable_watermarks: after.durable_watermarks.clone(),
                    transition: fact.transition.clone(),
                }),
            });
        }
    }
    Ok(result)
}

fn submission_map(
    conversation: &RelConversationSummary,
) -> BTreeMap<ConversationSubmissionId, &SubmissionObservation> {
    conversation
        .submissions
        .iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect()
}
fn branch_map(conversation: &RelConversationSummary) -> BTreeMap<String, &BranchObservation> {
    conversation
        .branches
        .iter()
        .map(|entry| (entry.branch_id.clone(), entry))
        .collect()
}
fn generation_map_for_conversation(
    conversation: &RelConversationSummary,
) -> BTreeMap<GenerationKey, &GenerationObservation> {
    conversation
        .generations
        .iter()
        .map(|entry| {
            (
                GenerationKey::new(&conversation.conversation_id, entry),
                entry,
            )
        })
        .collect()
}
fn host_operation_map(
    conversation: &RelConversationSummary,
) -> BTreeMap<(ConversationSubmissionId, String), &HostOperationObservation> {
    conversation
        .host_operations
        .iter()
        .map(|entry| {
            (
                (entry.submission_id.clone(), entry.operation_id.clone()),
                entry,
            )
        })
        .collect()
}

pub(super) fn generation_map(
    summary: &RelObservationSummary,
) -> BTreeMap<GenerationKey, &GenerationObservation> {
    summary
        .conversations
        .iter()
        .flat_map(|conversation| {
            conversation.generations.iter().map(move |generation| {
                (
                    GenerationKey::new(&conversation.conversation_id, generation),
                    generation,
                )
            })
        })
        .collect()
}
