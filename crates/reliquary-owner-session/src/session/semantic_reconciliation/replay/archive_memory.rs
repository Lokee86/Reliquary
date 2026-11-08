use super::*;
use reliquary_memory_core::PayloadState;

pub(super) fn replay_archive(
    target: &mut Container,
    source: &Archive,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    let mut destination = Archive::open(target).map_err(conflict)?;
    for accepted in source.accepted_turn_sources() {
        let node = source
            .node(&accepted.conversation_id, &accepted.node_id)
            .ok_or_else(|| conflict("accepted Archive source is absent"))?;
        let inserted = destination.node(&node.conversation_id, &node.id).is_none();
        let mut attachments = Vec::new();
        let mut project_attachments = Vec::new();
        for attachment in source.files_for_source(&node.conversation_id, &node.id) {
            if let Some(reference) = attachment.project_reference {
                project_attachments.push(reliquary_archive::ProjectAttachmentInput {
                    filename: attachment.filename,
                    mime_type: attachment.mime_type,
                    content_id: attachment.content_id,
                    byte_length: attachment.byte_length,
                    reference,
                });
            } else {
                attachments.push(reliquary_archive::AttachmentInput {
                    filename: attachment.filename,
                    mime_type: attachment.mime_type,
                    bytes: attachment.bytes,
                });
            }
        }
        destination
            .accept_turn(
                target,
                reliquary_archive::TurnInput {
                    id: node.id.clone(),
                    conversation_id: node.conversation_id.clone(),
                    parent_id: node.parent_id.clone(),
                    role: node.role.clone(),
                    principal_id: node.principal_id.clone(),
                    timestamp_ns: node.timestamp_ns,
                    content: source
                        .content_for_node(&node.conversation_id, &node.id)
                        .ok_or_else(|| conflict("Archive content is unavailable"))?
                        .to_owned(),
                    attachments,
                    project_attachments,
                },
            )
            .map_err(conflict)?;
        counts.archive += usize::from(inserted);
    }
    for branch in source.branches() {
        counts.archive += usize::from(destination.put_branch(target, branch).map_err(conflict)?);
    }
    for episode in source.episodes() {
        let inserted = destination.episode(episode.id).is_none();
        let id = destination
            .put_episode(
                target,
                &episode.conversation_id,
                &episode.start_node_id,
                &episode.end_node_id,
                episode.origin,
                episode.boundary,
                episode.finalized_at_ns,
            )
            .map_err(conflict)?;
        if id != episode.id {
            return Err(conflict("Episode stable identity changed"));
        }
        counts.archive += usize::from(inserted);
    }
    for receipt in source.import_receipts() {
        if destination.import_receipt(&receipt.import_key) == Some(&receipt) {
            // A retained receipt was checked at its accepted branch head.
            continue;
        }
        let before = destination.import_receipts().len();
        destination
            .put_import_receipt(target, receipt)
            .map_err(conflict)?;
        counts.archive += usize::from(destination.import_receipts().len() > before);
    }
    for definition in source.fragment_definitions() {
        let before = destination.fragment_definitions().len();
        destination
            .define_branch_fragments(
                target,
                &definition.fragment.conversation_id,
                &definition.branch_id,
                definition.config,
                definition.close_tail,
            )
            .map_err(conflict)?;
        counts.archive += usize::from(destination.fragment_definitions().len() > before);
    }
    Ok(())
}

pub(super) fn replay_memories(
    target: &mut Container,
    source: &MemoryStore,
    destination: &mut MemoryStore,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    for id in source.memory_ids() {
        let history = source.history(id).map_err(conflict)?;
        for revision in &history.revisions {
            let PayloadState::Available(body) = &revision.payload else {
                return Err(RelReconciliationError::UnsupportedState(
                    "pruned Memory payload in divergent history",
                ));
            };
            let published = destination
                .publish(
                    target,
                    Some(id),
                    revision.record.revision - 1,
                    memory_draft(revision, body),
                )
                .map_err(conflict)?;
            if published.published {
                counts.memory += 1;
            } else {
                counts.memory_duplicates += 1;
            }
        }
        if let Some(tombstone) = history.tombstone {
            let already = destination
                .history(id)
                .map_err(conflict)?
                .tombstone
                .is_some();
            destination
                .tombstone(target, id, tombstone.revision - 1)
                .map_err(conflict)?;
            counts.memory += usize::from(!already);
        }
    }
    Ok(())
}

pub(super) fn replay_echo(
    target: &mut Container,
    source: &reliquary_archive::EchoStore,
    counts: &mut ReplayCounts,
) -> Result<(), RelReconciliationError> {
    let mut destination = reliquary_archive::EchoStore::open(target).map_err(conflict)?;
    for event in source.all_records() {
        counts.archive += usize::from(destination.put(target, event).map_err(conflict)?);
    }
    Ok(())
}

fn memory_draft(
    revision: &reliquary_memory_core::MemoryRevision,
    body: &reliquary_memory_core::MemoryBody,
) -> MemoryDraft {
    let record = &revision.record;
    MemoryDraft {
        title: body.title.clone(),
        content: body.content.clone(),
        category: record.category.clone(),
        memory_type: record.memory_type.clone(),
        authority_kind: record.authority_kind.clone(),
        temporal_status: record.temporal_status.clone(),
        scope: record.scope.clone(),
        lifecycle_state: record.lifecycle_state.clone(),
        archived: record.archived,
        parent_id: record.parent_id,
        superseded_by: record.superseded_by,
        source_node_id: record.source_node_id.clone(),
        content_source_conversation_id: record.content_source_conversation_id.clone(),
        content_source_node_id: record.content_source_node_id.clone(),
        grounding_source_conversation_id: record.grounding_source_conversation_id.clone(),
        grounding_source_node_id: record.grounding_source_node_id.clone(),
        source_episode_id: record.source_episode_id,
        source_ref: record.source_ref.clone(),
        source_time_ns: record.source_time_ns,
        temporal_inference: record.temporal_inference.clone(),
        mutation_id: record.mutation_id.clone(),
        created_at_ns: record.created_at_ns,
        updated_at_ns: record.updated_at_ns,
    }
}
