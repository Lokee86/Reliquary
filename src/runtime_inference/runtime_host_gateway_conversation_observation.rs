//! Authoritative P3 observations; P4 owns registration, cursors and delivery.
use super::super::observation::{
    BranchObservation, CheckpointContentIdentity, ConversationDurableDelta,
    ConversationObservationSnapshot, ConversationTransition, GenerationCheckpointContent,
    GenerationCheckpointObservation, GenerationIdentity, GenerationObservation, GenerationStatus,
    HostOperationObservation, RelConversationSummary, RelObservationSummary, SubmissionObservation,
    SubmissionOrder,
};
use super::*;
use crate::multiplexing_journal::RelJournal;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const MAX_CONVERSATION_SOURCE_ITEMS: usize = 16_384;
const MAX_DURABLE_STREAM_PREFIX_BYTES: usize = 1024 * 1024;

impl GatewayRuntime {
    /// Existing P3 read remains grant-checked and owner-admitted, outside the P4 gate.
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
            runtime.cva().sync().map_err(operation)?;
            coordinator.observe(&journal, |journal| {
                build_conversation_observation(
                    runtime,
                    journal,
                    owner_id,
                    token.mount_epoch,
                    &conversation_id,
                    Some(instance),
                )
            })
        })?;
        with_admission(self, &token, || Ok(snapshot))
    }

    /// P4-gated broad source. This method does not reacquire the P4 publication
    /// gate or owner admission. REL summaries intentionally omit instance selection.
    pub(in super::super) fn read_conversation_distribution_source(
        &self,
        owner_id: &str,
        epoch: u64,
        instance: Option<&InstanceRuntime>,
    ) -> Result<RelObservationSummary, ReliquaryRuntimeHostError> {
        let _ = instance; // Selection is exposed only by the exact conversation stream.
        self.with_conversation_source_runtime(owner_id, epoch, |runtime| {
            runtime.cva().sync().map_err(operation)?;
            let journal = self.conversation.journal(owner_id)?;
            let cva = runtime.cva();
            // Clone the body-free journal index once, then partition it by
            // conversation. The same journal cut is checked while each
            // coordinator observation is acquired below.
            let (statuses, journal_sequence) = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let statuses = journal.statuses();
                if statuses.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                    return Err(operation("REL conversation journal exceeds its item bound"));
                }
                (statuses, journal.sequence())
            };
            let mut statuses_by_conversation = BTreeMap::new();
            for (id, status) in statuses {
                statuses_by_conversation
                    .entry(id.conversation_id.clone())
                    .or_insert_with(Vec::new)
                    .push((id, status));
            }
            let mut ids = BTreeSet::new();
            ids.extend(
                cva.conversation_summaries()
                    .into_iter()
                    .map(|s| s.conversation_id),
            );
            // Node-derived summaries omit metadata-only conversations.
            ids.extend(
                cva.archive()
                    .conversations
                    .iter()
                    .map(|m| m.conversation_id.clone()),
            );
            ids.extend(
                cva.branches()
                    .into_iter()
                    .map(|branch| branch.conversation_id),
            );
            ids.extend(
                statuses_by_conversation
                    .keys()
                    .map(|id| id.as_str().to_owned()),
            );
            if ids.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                return Err(operation(
                    "REL conversation observation exceeds its item bound",
                ));
            }
            let mut conversations = Vec::with_capacity(ids.len());
            let mut count = 0usize;
            for id in ids {
                let conversation_id = ConversationId::new(id).map_err(operation)?;
                let coordinator = self
                    .conversation
                    .coordinator(owner_id, conversation_id.as_str())?;
                let conversation_statuses = statuses_by_conversation
                    .remove(&conversation_id)
                    .unwrap_or_default();
                let summary = coordinator.observe(&journal, |journal| {
                    if journal.sequence() != journal_sequence {
                        return Err(operation(
                            "REL journal changed during conversation source capture",
                        ));
                    }
                    let snapshot = build_conversation_observation_from_statuses(
                        runtime,
                        journal,
                        owner_id,
                        epoch,
                        &conversation_id,
                        conversation_statuses,
                        None,
                    )?;
                    count = count
                        .checked_add(
                            1 + snapshot.branches.len()
                                + snapshot.submissions.len()
                                + snapshot.generations.len()
                                + snapshot.host_operations.len(),
                        )
                        .ok_or_else(|| operation("REL conversation observation count overflow"))?;
                    if count > MAX_CONVERSATION_SOURCE_ITEMS {
                        return Err(operation(
                            "REL conversation observation exceeds its item bound",
                        ));
                    }
                    Ok(RelConversationSummary {
                        conversation_id: snapshot.conversation_id,
                        policy: snapshot.policy,
                        policy_revision: snapshot.policy_revision,
                        branches: snapshot.branches,
                        submissions: snapshot.submissions,
                        generations: snapshot.generations,
                        host_operations: snapshot.host_operations,
                    })
                })?;
                conversations.push(summary);
            }
            let mut durable_watermarks =
                super::super::snapshots::reader::source_watermarks(runtime.cva());
            let rel_id = {
                let journal = journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                if journal.sequence() != journal_sequence {
                    return Err(operation(
                        "REL journal changed during conversation source capture",
                    ));
                }
                journal.rel_id()
            };
            durable_watermarks.conversation_journal_sequence = Some(journal_sequence);
            Ok(RelObservationSummary {
                rel_id,
                mount_epoch: epoch,
                conversations,
                durable_watermarks,
            })
        })
    }

    /// Return the journal-authoritative full checkpoint prefix for one generation.
    /// Stream bytes beyond the receipt are truncated and never exposed.
    pub(in super::super) fn read_conversation_checkpoint(
        &self,
        owner_id: &str,
        epoch: u64,
        conversation_id: &ConversationId,
        generation: &GenerationObservation,
    ) -> Result<Option<GenerationCheckpointContent>, ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(owner_id)?;
        let coordinator = self
            .conversation
            .coordinator(owner_id, conversation_id.as_str())?;
        self.with_conversation_source_runtime(owner_id, epoch, |runtime| {
            coordinator.observe(&journal, |journal| {
                checkpoint_from_journal(runtime, journal, epoch, conversation_id, generation)
            })
        })
    }

    /// Exact P4 stream source. Caller already holds P4 admission and gate.
    pub(in super::super) fn read_conversation_stream_source(
        &self,
        instance: &InstanceRuntime,
        key: &super::super::streams::StreamKey,
    ) -> Result<super::super::streams::StreamSourceSnapshot, ReliquaryRuntimeHostError> {
        let owner_id = format!("rel-{}", key.rel_id.as_uuid());
        if key.mount_epoch == 0 {
            return Err(operation("stream source requires a mounted REL epoch"));
        }
        let conversation_id = key.conversation_id.clone();
        let journal = self.conversation.journal(&owner_id)?;
        let coordinator = self
            .conversation
            .coordinator(&owner_id, conversation_id.as_str())?;
        self.with_conversation_source_runtime(&owner_id, key.mount_epoch, |runtime| {
            runtime.cva().sync().map_err(operation)?;
            coordinator.observe(&journal, |journal| {
                let conversation = build_conversation_observation(
                    runtime,
                    journal,
                    &owner_id,
                    key.mount_epoch,
                    &conversation_id,
                    Some(instance),
                )?;
                let mut matches = conversation.generations.iter().filter(|g| {
                    g.identity.operation_id == key.operation_id
                        && g.identity.generation_id == key.generation_id
                        && g.identity.branch_id == key.branch_id
                });
                let generation = matches
                    .next()
                    .ok_or_else(|| operation("stream generation is absent from P3 observation"))?;
                if matches.next().is_some() {
                    return Err(operation("duplicate generation identity in P3 observation"));
                }
                let checkpoint = checkpoint_from_journal(
                    runtime,
                    journal,
                    key.mount_epoch,
                    &conversation_id,
                    generation,
                )?;
                Ok(super::super::streams::StreamSourceSnapshot {
                    conversation,
                    checkpoint,
                })
            })
        })
    }

    /// Body-free checkpoint identities for P4 before/after comparison. The journal
    /// receipt remains authoritative; no historic stream text is opened here.
    pub(in super::super) fn read_conversation_checkpoint_positions(
        &self,
        owner_id: &str,
        epoch: u64,
    ) -> Result<Vec<GenerationCheckpointObservation>, ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(owner_id)?;
        self.with_conversation_source_runtime(owner_id, epoch, |_runtime| {
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel_id = journal.rel_id();
            let mut positions = Vec::new();
            let statuses = journal.statuses();
            if statuses.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                return Err(operation("REL checkpoint journal exceeds its item bound"));
            }
            for (id, status) in statuses {
                let Some(generation) = status.generation else {
                    continue;
                };
                let Some(checkpoint) = generation.checkpoint else {
                    continue;
                };
                positions.push(GenerationCheckpointObservation {
                    rel_id,
                    mount_epoch: epoch,
                    conversation_id: id.conversation_id,
                    generation: GenerationIdentity {
                        operation_id: generation.operation_id,
                        generation_id: generation.generation_id,
                        branch_id: generation.branch_id,
                    },
                    durable_checkpoint_sequence: checkpoint.sequence,
                    status: generation_status(generation.phase),
                    content: CheckpointContentIdentity::Prefix {
                        utf8_bytes: checkpoint.utf8_bytes,
                        sha256: checkpoint.sha256,
                    },
                });
                if positions.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                    return Err(operation(
                        "REL checkpoint positions exceed their item bound",
                    ));
                }
            }
            Ok(positions)
        })
    }

    /// Capture only named durability facts proven by existing journal captures/receipts.
    pub(in super::super) fn read_conversation_receipt_facts(
        &self,
        owner_id: &str,
        epoch: u64,
    ) -> Result<Vec<ConversationDurableDelta>, ReliquaryRuntimeHostError> {
        let journal = self.conversation.journal(owner_id)?;
        self.with_conversation_source_runtime(owner_id, epoch, |runtime| {
            runtime.cva().sync().map_err(operation)?;
            let journal = journal
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let mut watermarks = super::super::snapshots::reader::source_watermarks(runtime.cva());
            watermarks.conversation_journal_sequence = Some(journal.sequence());
            let rel_id = journal.rel_id();
            let mut facts = Vec::new();
            let statuses = journal.statuses();
            if statuses.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                return Err(operation("REL receipt journal exceeds its item bound"));
            }
            // Branch origin is immutable capture metadata. A later receipt may
            // continue a branch, so its receipt parent must not replace the
            // original fork parent. Match build_conversation_observation's
            // branch metadata rule: the first labeled finalize capture owns it.
            let mut branch_origins = BTreeMap::new();
            for (submission_id, status) in &statuses {
                let Some(capture) = status
                    .finalize_capture
                    .as_ref()
                    .filter(|capture| capture.branch_label.is_some())
                else {
                    continue;
                };
                branch_origins
                    .entry((
                        submission_id.conversation_id.as_str().to_owned(),
                        capture.branch_id.clone(),
                    ))
                    .or_insert_with(|| {
                        (capture.parent_node_id.clone(), capture.branch_label.clone())
                    });
            }
            for (id, status) in statuses {
                if let Some(capture) = status.finalize_capture.as_ref() {
                    let (fork_parent_node_id, display_label) = branch_origins
                        .get(&(
                            id.conversation_id.as_str().to_owned(),
                            capture.branch_id.clone(),
                        ))
                        .cloned()
                        .unwrap_or((None, None));
                    facts.push(ConversationDurableDelta {
                        rel_id,
                        mount_epoch: epoch,
                        conversation_id: id.conversation_id.clone(),
                        durable_watermarks: watermarks.clone(),
                        transition: ConversationTransition::FinalizeWon {
                            submission_id: id.clone(),
                            branch: BranchObservation {
                                branch_id: capture.branch_id.clone(),
                                head_node_id: None,
                                fork_parent_node_id,
                                display_label,
                            },
                            policy_revision: capture.policy_revision,
                        },
                    });
                }
                if let Some(receipt) = status.archive_receipt.as_ref() {
                    let (fork_parent_node_id, display_label) = branch_origins
                        .get(&(
                            id.conversation_id.as_str().to_owned(),
                            receipt.branch_id.clone(),
                        ))
                        .cloned()
                        .unwrap_or((None, None));
                    facts.push(ConversationDurableDelta {
                        rel_id,
                        mount_epoch: epoch,
                        conversation_id: id.conversation_id.clone(),
                        durable_watermarks: watermarks.clone(),
                        transition: ConversationTransition::ArchiveCommitted {
                            submission_id: id.clone(),
                            branch: BranchObservation {
                                branch_id: receipt.branch_id.clone(),
                                head_node_id: Some(receipt.node_id.clone()),
                                fork_parent_node_id,
                                display_label,
                            },
                            node_id: receipt.node_id.clone(),
                        },
                    });
                }
                if status.post_commit_receipt.is_some() {
                    facts.push(ConversationDurableDelta {
                        rel_id,
                        mount_epoch: epoch,
                        conversation_id: id.conversation_id.clone(),
                        durable_watermarks: watermarks.clone(),
                        transition: ConversationTransition::PostCommitRecorded {
                            submission_id: id,
                        },
                    });
                }
                if facts.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                    return Err(operation(
                        "REL durable receipt facts exceed their item bound",
                    ));
                }
            }
            if facts.len() > MAX_CONVERSATION_SOURCE_ITEMS {
                return Err(operation(
                    "REL durable receipt facts exceed their item bound",
                ));
            }
            Ok(facts)
        })
    }

    /// Mirrors existing owner -> InteractionRuntime acquisition. No second gate or lease.
    fn with_conversation_source_runtime<T>(
        &self,
        owner_id: &str,
        epoch: u64,
        read: impl FnOnce(&mut crate::InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let (owner, mounted_epoch) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            (
                Arc::clone(&rel.owner),
                rel.mount_epoch.load(std::sync::atomic::Ordering::SeqCst),
            )
        };
        if mounted_epoch != epoch {
            return Err(operation(
                "conversation source belongs to a previous REL mount epoch",
            ));
        }
        let owner = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let runtime = owner
            .runtime
            .as_ref()
            .cloned()
            .ok_or_else(|| operation("REL runtime is unavailable"))?;
        drop(owner);
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        read(&mut runtime)
    }
}

fn build_conversation_observation(
    runtime: &crate::InteractionRuntime,
    journal: &RelJournal,
    owner_id: &str,
    mount_epoch: u64,
    conversation_id: &ConversationId,
    instance: Option<&InstanceRuntime>,
) -> Result<ConversationObservationSnapshot, ReliquaryRuntimeHostError> {
    let statuses = journal.statuses();
    if statuses.len() > MAX_CONVERSATION_SOURCE_ITEMS {
        return Err(operation("REL conversation journal exceeds its item bound"));
    }
    let statuses = statuses
        .into_iter()
        .filter(|(id, _)| id.conversation_id == *conversation_id)
        .collect();
    build_conversation_observation_from_statuses(
        runtime,
        journal,
        owner_id,
        mount_epoch,
        conversation_id,
        statuses,
        instance,
    )
}

fn build_conversation_observation_from_statuses(
    runtime: &crate::InteractionRuntime,
    journal: &RelJournal,
    owner_id: &str,
    mount_epoch: u64,
    conversation_id: &ConversationId,
    statuses: Vec<(ConversationSubmissionId, SubmissionStatus)>,
    instance: Option<&InstanceRuntime>,
) -> Result<ConversationObservationSnapshot, ReliquaryRuntimeHostError> {
    let (policy, policy_revision) = runtime.cva().conversation_policy(conversation_id.as_str());
    let archive = runtime.cva().archive();
    let mut branches: Vec<_> = archive
        .branches()
        .into_iter()
        .filter(|b| b.conversation_id == conversation_id.as_str())
        .map(|branch| {
            let capture = statuses
                .iter()
                .filter_map(|(_, s)| s.finalize_capture.as_ref())
                .find(|c| c.branch_id == branch.id && c.branch_label.is_some());
            BranchObservation {
                branch_id: branch.id,
                head_node_id: Some(branch.leaf_node_id),
                fork_parent_node_id: capture.and_then(|c| c.parent_node_id.clone()),
                display_label: capture.and_then(|c| c.branch_label.clone()),
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
        if let Some(g) = status.generation {
            generations.push(GenerationObservation {
                identity: GenerationIdentity {
                    operation_id: g.operation_id,
                    generation_id: g.generation_id,
                    branch_id: g.branch_id,
                },
                status: generation_status(g.phase),
                transition_revision: g.transition_revision,
                durable_checkpoint_sequence: g.checkpoint.map(|c| c.sequence),
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
    submissions.sort_by_key(|s| s.order);
    generations.sort_by(|a, b| {
        (&a.identity.branch_id, &a.identity.operation_id)
            .cmp(&(&b.identity.branch_id, &b.identity.operation_id))
    });
    host_operations.sort_by(|a, b| {
        (&a.submission_id, &a.operation_id).cmp(&(&b.submission_id, &b.operation_id))
    });
    if [
        branches.len(),
        submissions.len(),
        generations.len(),
        host_operations.len(),
    ]
    .into_iter()
    .any(|n| n > MAX_CONVERSATION_SOURCE_ITEMS)
    {
        return Err(operation("conversation observation exceeds its item bound"));
    }
    let selection = instance
        .map(|i| i.managed_conversation_state(owner_id).map_err(operation))
        .transpose()?;
    let selected_branch_id = selection.and_then(|s| {
        if s.conversation_id.as_deref() == Some(conversation_id.as_str()) {
            s.branch_id
        } else {
            None
        }
    });
    let selected_head_node_id = selected_branch_id
        .as_ref()
        .and_then(|id| branches.iter().find(|b| &b.branch_id == id))
        .and_then(|b| b.head_node_id.clone());
    Ok(ConversationObservationSnapshot {
        rel_id: journal.rel_id(),
        mount_epoch,
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
}

fn checkpoint_from_journal(
    runtime: &crate::InteractionRuntime,
    journal: &RelJournal,
    mount_epoch: u64,
    conversation_id: &ConversationId,
    observed: &GenerationObservation,
) -> Result<Option<GenerationCheckpointContent>, ReliquaryRuntimeHostError> {
    let mut matches = journal.statuses().into_iter().filter_map(|(id, status)| {
        if id.conversation_id != *conversation_id {
            return None;
        }
        status.generation.filter(|g| {
            g.operation_id == observed.identity.operation_id
                && g.generation_id == observed.identity.generation_id
                && g.branch_id == observed.identity.branch_id
        })
    });
    let generation = matches
        .next()
        .ok_or_else(|| operation("durable generation is absent from the REL journal"))?;
    if matches.next().is_some()
        || generation.transition_revision != observed.transition_revision
        || generation_status(generation.phase) != observed.status
    {
        return Err(operation(
            "generation observation no longer matches its durable journal identity",
        ));
    }
    let Some(checkpoint) = generation.checkpoint.as_ref() else {
        if observed.durable_checkpoint_sequence.is_some() {
            return Err(operation(
                "journal checkpoint is absent from observed generation",
            ));
        }
        return Ok(None);
    };
    if observed.durable_checkpoint_sequence != Some(checkpoint.sequence) {
        return Err(operation(
            "checkpoint sequence differs from the authoritative journal receipt",
        ));
    }
    // Borrow the exact stream record by its durable generation identity. The
    // session-wide convenience accessor clones every record and its full text;
    // doing that here would read unbounded ahead text while holding source locks.
    let record = runtime
        .cva()
        .interaction_stream_record(&generation.generation_id)
        .ok_or_else(|| operation("durable checkpoint has no interaction stream record"))?;
    if record.session_id != conversation_id.as_str()
        || record.message_id != generation.generation_id
        || record.parent_message_id.as_deref() != Some(generation.parent_node_id.as_str())
        || record.role != crate::InteractionRole::Agent
        || record.timestamp_ns != generation.started_timestamp_ns
    {
        return Err(operation(
            "generation interaction-stream identity differs from durable generation",
        ));
    }
    let byte_len = usize::try_from(checkpoint.utf8_bytes)
        .map_err(|_| operation("checkpoint byte length exceeds addressable memory"))?;
    if byte_len > MAX_DURABLE_STREAM_PREFIX_BYTES || byte_len > record.content.len() {
        return Err(operation(
            "durable checkpoint prefix exceeds its available stream bounds",
        ));
    }
    let prefix = record
        .content
        .as_bytes()
        .get(..byte_len)
        .ok_or_else(|| operation("checkpoint byte length splits a UTF-8 code point"))?;
    let text = std::str::from_utf8(prefix)
        .map_err(|_| operation("checkpoint prefix is not valid UTF-8"))?
        .to_owned();
    let digest: [u8; 32] = Sha256::digest(prefix).into();
    if digest != checkpoint.sha256 {
        return Err(operation(
            "interaction stream prefix differs from the durable checkpoint hash",
        ));
    }
    Ok(Some(GenerationCheckpointContent {
        observation: GenerationCheckpointObservation {
            rel_id: journal.rel_id(),
            mount_epoch,
            conversation_id: conversation_id.clone(),
            generation: observed.identity.clone(),
            durable_checkpoint_sequence: checkpoint.sequence,
            status: observed.status,
            content: CheckpointContentIdentity::Prefix {
                utf8_bytes: checkpoint.utf8_bytes,
                sha256: checkpoint.sha256,
            },
        },
        text,
    }))
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
