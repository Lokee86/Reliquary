//! Validated ordered Memory and Insomnia replay over explicit owner parts.
use crate::insomnia::completion::{InsomniaCompletion, decode_completion, encode_completion};
use crate::insomnia::store::InsomniaStore;
use crate::memory_compact_codec::publication::coordinates as memory_publication_coordinates;
use crate::memory_store::MemoryStore;
use crate::{
    Archive, Container, CvaReconcileConflict, CvaReconcileError, InsomniaAttempt,
    InsomniaWorkState, Memory, MemoryDraft, MemoryError,
};
use std::collections::HashSet;

pub(crate) struct MemoryReplaySource<'a> {
    pub(crate) container: &'a Container,
    pub(crate) memories: &'a MemoryStore,
}

pub(crate) struct MemoryReplayDestination<'a> {
    pub(crate) archive: &'a Archive,
    pub(crate) memories: &'a mut MemoryStore,
    pub(crate) insomnia: &'a mut InsomniaStore,
    pub(crate) container: &'a mut Container,
}

pub(crate) struct MemoryTail {
    revisions: Vec<MemoryReplayRevision>,
    completions: Vec<InsomniaCompletion>,
}

struct MemoryReplayRevision {
    memory: Memory,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct MemoryReplayResult {
    pub(crate) revisions: usize,
    pub(crate) duplicate_revisions: usize,
    pub(crate) completions: usize,
}

/// Read a finite source interval without moving later epochs ahead of it.
pub(crate) fn read_memory_range(
    mut source: MemoryReplaySource<'_>,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<MemoryTail, CvaReconcileError> {
    let chunks = source.container.chunk_refs()?;
    let mut revisions = Vec::new();
    let mut completions = Vec::new();
    let mut seen = HashSet::new();

    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = source.container.read(*chunk)?;
        if let Some(completion) =
            decode_completion(&payload).map_err(CvaReconcileError::InvalidInsomniaCompletion)?
        {
            for record in &completion.records {
                push_revision(
                    &mut source,
                    record.id,
                    record.revision,
                    &mut seen,
                    &mut revisions,
                )?;
            }
            completions.push(completion);
            continue;
        }
        let Some((global_version, memory_version)) = memory_publication_coordinates(&payload)?
        else {
            continue;
        };
        let record = source
            .memories
            .record_at_publication(global_version, memory_version)
            .ok_or(CvaReconcileError::InvalidMemoryVersionRecord)?;
        let (id, revision) = (record.id, record.revision);
        push_revision(&mut source, id, revision, &mut seen, &mut revisions)?;
    }

    Ok(MemoryTail {
        revisions,
        completions,
    })
}

pub(crate) fn replay_memory_tail(
    mut destination: MemoryReplayDestination<'_>,
    tail: MemoryTail,
) -> Result<MemoryReplayResult, CvaReconcileError> {
    let mut replayed = 0;
    let mut duplicates = 0;
    for revision in tail.revisions {
        let memory = revision.memory;
        let draft = memory_draft(memory.clone());
        let source_ref = memory.source_ref.clone();
        let temporal_inference = memory.temporal_inference.clone();
        let routing_metadata = memory.routing_metadata.clone();

        destination
            .container
            .set_next_transaction_time_override(revision.transaction_time_ns);
        let publish = crate::cva_memory_publish::publish_memory_parts_with_state(
            destination.archive,
            destination.memories,
            destination.container,
            Some(memory.id),
            memory.revision.saturating_sub(1),
            draft,
            source_ref,
            temporal_inference,
        );
        destination.container.clear_next_transaction_time_override();
        let (_, created) = match publish {
            Ok(value) => value,
            Err(error) => {
                return Err(memory_replay_error(
                    destination.container,
                    destination.memories,
                    &memory,
                    error,
                ));
            }
        };
        if let Some(metadata) = routing_metadata {
            destination
                .memories
                .put_routing_metadata(destination.container, metadata)?;
        }
        if created {
            replayed += 1;
        } else {
            duplicates += 1;
        }
    }

    let mut completions = 0;
    for completion in tail.completions {
        if replay_completion(&mut destination, completion)? {
            completions += 1;
        }
    }
    Ok(MemoryReplayResult {
        revisions: replayed,
        duplicate_revisions: duplicates,
        completions,
    })
}

fn push_revision(
    source: &mut MemoryReplaySource<'_>,
    id: crate::MemoryId,
    revision: u64,
    seen: &mut HashSet<(crate::MemoryId, u64)>,
    revisions: &mut Vec<MemoryReplayRevision>,
) -> Result<(), CvaReconcileError> {
    if seen.insert((id, revision)) {
        // Preserve the prior facade error envelope for a missing retained body.
        let memory = source
            .memories
            .memory_revision(source.container, id, revision)?;
        let transaction_time_ns = source.container.transaction_time_ns(memory.global_version);
        revisions.push(MemoryReplayRevision {
            memory,
            transaction_time_ns,
        });
    }
    Ok(())
}

fn replay_completion(
    destination: &mut MemoryReplayDestination<'_>,
    completion: InsomniaCompletion,
) -> Result<bool, CvaReconcileError> {
    let existing = destination.insomnia.attempts(completion.episode_id);
    if !existing.is_empty() {
        if existing.len() == 1 && same_completion(&existing[0], &completion) {
            return Ok(false);
        }
        let existing = &existing[0];
        return Err(CvaReconcileError::Conflict(
            CvaReconcileConflict::InsomniaCompletion {
                episode_id: completion.episode_id,
                existing_model: existing.extractor_model.clone(),
                existing_version: existing.extractor_version.clone(),
                incoming_model: completion.extractor_model.clone(),
                incoming_version: completion.extractor_version.clone(),
            },
        ));
    }
    if completion
        .memory_ids
        .iter()
        .any(|id| !destination.memories.contains_memory(*id))
    {
        return Err(CvaReconcileError::MissingCompletionMemory);
    }

    let receipt_transaction_time_ns = if completion.transaction_time_ns == 0 {
        Container::transaction_time_now_ns()?
    } else {
        completion.transaction_time_ns
    };
    let receipt = InsomniaCompletion {
        episode_id: completion.episode_id,
        attempt: completion.attempt,
        started_at_ns: completion.started_at_ns,
        completed_at_ns: completion.completed_at_ns,
        transaction_time_ns: receipt_transaction_time_ns,
        extractor_model: completion.extractor_model,
        extractor_version: completion.extractor_version,
        rejected_count: completion.rejected_count,
        memory_ids: completion.memory_ids,
        external_memory_refs: completion.external_memory_refs,
        global_version_start: destination.container.next_version_candidate(),
        bodies: Vec::new(),
        records: Vec::new(),
        routing_metadata: Vec::new(),
        freshness_birth_turn: completion.freshness_birth_turn,
        freshness_policy_version: completion.freshness_policy_version,
        freshness_birth_memory_ids: completion.freshness_birth_memory_ids,
    };
    let payload =
        encode_completion(&receipt).map_err(CvaReconcileError::InvalidInsomniaCompletion)?;
    destination.container.append(&payload)?;
    destination
        .insomnia
        .apply_completion(&crate::owner_lifecycle::insomnia_replay::completion_receipt(&receipt))?;
    Ok(true)
}

fn memory_replay_error(
    container: &Container,
    memories: &MemoryStore,
    memory: &Memory,
    error: MemoryError,
) -> CvaReconcileError {
    let conflict = match error {
        MemoryError::RevisionConflict => {
            let current_revision = memories
                .memory(container, memory.id)
                .map(|current| current.revision)
                .unwrap_or(0);
            Some(CvaReconcileConflict::MemoryRevision {
                memory_id: memory.id,
                expected_revision: memory.revision.saturating_sub(1),
                current_revision,
                incoming_revision: memory.revision,
            })
        }
        MemoryError::MutationConflict => Some(CvaReconcileConflict::MemoryMutation {
            memory_id: memory.id,
            mutation_id: memory.mutation_id.clone(),
            incoming_revision: memory.revision,
        }),
        MemoryError::SemanticMutation => Some(CvaReconcileConflict::MemorySemanticMutation {
            memory_id: memory.id,
            incoming_revision: memory.revision,
        }),
        other => return CvaReconcileError::Memories(other),
    };
    CvaReconcileError::Conflict(conflict.expect("mapped Memory conflict"))
}

fn same_completion(attempt: &InsomniaAttempt, completion: &InsomniaCompletion) -> bool {
    attempt.state == InsomniaWorkState::Complete
        && attempt.attempt == completion.attempt
        && attempt.started_at_ns == completion.started_at_ns
        && attempt.completed_at_ns == completion.completed_at_ns
        && attempt.extractor_model == completion.extractor_model
        && attempt.extractor_version == completion.extractor_version
        && attempt.memory_ids == completion.memory_ids
        && attempt.external_memory_refs == completion.external_memory_refs
        && attempt.rejected_count == completion.rejected_count
}

fn memory_draft(memory: Memory) -> MemoryDraft {
    MemoryDraft {
        category: memory.category,
        memory_type: memory.memory_type,
        authority_kind: memory.authority_kind,
        temporal_status: memory.temporal_status,
        title: memory.title,
        content: memory.content,
        scope: memory.scope,
        lifecycle_state: memory.lifecycle_state,
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id,
        content_source_conversation_id: memory.content_source_conversation_id,
        content_source_node_id: memory.content_source_node_id,
        grounding_source_conversation_id: memory.grounding_source_conversation_id,
        grounding_source_node_id: memory.grounding_source_node_id,
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: memory.mutation_id,
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns,
    }
}
