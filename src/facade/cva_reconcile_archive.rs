use crate::archive_codec::ArchiveRecord;
use crate::cva_reconcile::with_replayed_transaction_time;
use crate::cva_reconcile_conflict_map::archive_replay_error;
use crate::{
    ArchiveError, Branch, ConversationMetadata, Cva, CvaReconcileError, Episode, FileMemoryLink,
    Fragment, IncomingAttachment, IncomingTurn, ProjectFileRef, StoredFile,
};

pub(crate) enum ArchiveReplayRecord {
    ImportReceipt(crate::imports::ImportReceipt),
    Node(IncomingTurn),
    IngestedTurn(IncomingTurn),
    Branch(Branch),
    ConversationMetadata(ConversationMetadata),
    ConversationTombstone(crate::ConversationTombstone),
    Episode(Episode),
    Fragment(Fragment),
    File(StoredFile, Option<Vec<u8>>, Option<ProjectFileRef>),
}

struct ArchiveReplayEntry {
    record: ArchiveReplayRecord,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct FileMemoryLinkReplay {
    link: FileMemoryLink,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct ArchiveTail {
    records: Vec<ArchiveReplayEntry>,
    pub(crate) file_memory_links: Vec<FileMemoryLinkReplay>,
}

pub(crate) fn rewrite_archive_conversations(
    source: &mut Cva,
    memories: &[crate::Memory],
    selection: &crate::rewrite::ArchiveSelection,
) -> Result<std::collections::BTreeSet<String>, CvaReconcileError> {
    use std::collections::BTreeSet;

    let mut conversations = BTreeSet::new();
    for memory in memories {
        for conversation in [
            memory.content_source_conversation_id.as_deref(),
            memory.grounding_source_conversation_id.as_deref(),
            memory
                .source_ref
                .as_ref()
                .and_then(|source| source.content_source_conversation_id.as_deref()),
            memory
                .source_ref
                .as_ref()
                .and_then(|source| source.grounding_source_conversation_id.as_deref()),
        ]
        .into_iter()
        .flatten()
        {
            conversations.insert(conversation.to_owned());
        }
        if let Some(episode_id) = memory.source_episode_id {
            let episode = source.historical_episode(episode_id).ok_or(
                CvaReconcileError::UnsupportedSemanticOwner(
                    "retained Memory source Episode is unavailable",
                ),
            )?;
            conversations.insert(episode.conversation_id.clone());
        }
        if let Some(source_ref) = &memory.source_ref {
            if let Some(episode) = source.historical_episode(source_ref.source_episode_id) {
                conversations.insert(episode.conversation_id.clone());
            }
        }
    }
    match selection {
        crate::rewrite::ArchiveSelection::ProvenanceClosureOnly => {}
        crate::rewrite::ArchiveSelection::AllRetained => {
            conversations.extend(
                source
                    .historical_episodes()
                    .into_iter()
                    .map(|episode| episode.conversation_id),
            );
            conversations.extend(
                source
                    .historical_branches()
                    .into_iter()
                    .map(|branch| branch.conversation_id),
            );
            conversations.extend(
                source
                    .conversation_summaries()
                    .into_iter()
                    .map(|summary| summary.conversation_id),
            );
        }
        crate::rewrite::ArchiveSelection::CurrentFromActivity(cut) => {
            for summary in source.conversation_summaries() {
                let mut has_recent = false;
                for leaf in &summary.leaf_node_ids {
                    let turns = source
                        .historical_conversation_turns(&summary.conversation_id, leaf)
                        .map_err(CvaReconcileError::from)?;
                    if turns.iter().any(|turn| {
                        source
                            .activity_position_for_turn(&summary.conversation_id, &turn.node_id)
                            .is_some_and(|position| position >= *cut)
                    }) {
                        has_recent = true;
                        break;
                    }
                }
                if has_recent {
                    conversations.insert(summary.conversation_id);
                }
            }
        }
        crate::rewrite::ArchiveSelection::Conversations(ids) => {
            conversations.extend(ids.iter().cloned());
        }
    }

    Ok(conversations)
}

/// Copy only the Archive history selected by a rewrite and the conversations
/// needed to support retained Memory provenance. This lives beside the replay
/// representation so filtering can happen before any source history is emitted.
pub(crate) fn replay_rewrite_archive(
    source: &mut Cva,
    destination: &mut Cva,
    memories: &[crate::Memory],
    selection: &crate::rewrite::ArchiveSelection,
) -> Result<usize, CvaReconcileError> {
    use std::collections::{BTreeSet, HashSet};
    let retained_ids: BTreeSet<_> = memories.iter().map(|memory| memory.id).collect();
    let conversations = rewrite_archive_conversations(source, memories, selection)?;
    let mut tail = read_archive_tail(source, 0)?;
    let mut selected_file_ids = HashSet::new();
    for entry in &tail.records {
        if let ArchiveReplayRecord::IngestedTurn(turn) = &entry.record
            && conversations.contains(&turn.conversation_id)
        {
            selected_file_ids.extend(turn.project_attachments.iter().map(|file| file.id));
        }
    }
    tail.file_memory_links.retain(|entry| {
        let keep = retained_ids.contains(&entry.link.memory_id);
        if keep {
            selected_file_ids.insert(entry.link.file_id);
        }
        keep
    });
    let mut file_ids: Vec<_> = selected_file_ids.iter().copied().collect();
    file_ids.sort_by_key(|id| id.0);
    for id in file_ids {
        let file = source.file(id).cloned().ok_or(ArchiveError::MissingFile)?;
        if let Some(reference) = source.project_file_ref(id) {
            replay_project_file(destination, &file, reference)?;
        } else {
            let bytes = source.file_bytes(id)?;
            destination.store_file(file.filename, file.mime_type, &bytes)?;
        }
    }
    tail.records.retain(|entry| match &entry.record {
        ArchiveReplayRecord::Node(turn) | ArchiveReplayRecord::IngestedTurn(turn) => {
            conversations.contains(&turn.conversation_id)
        }
        ArchiveReplayRecord::Branch(branch) => conversations.contains(&branch.conversation_id),
        ArchiveReplayRecord::ConversationMetadata(metadata) => {
            conversations.contains(&metadata.conversation_id)
        }
        ArchiveReplayRecord::ConversationTombstone(tombstone) => {
            conversations.contains(&tombstone.conversation_id)
        }
        ArchiveReplayRecord::Episode(episode) => conversations.contains(&episode.conversation_id),
        ArchiveReplayRecord::Fragment(fragment) => {
            conversations.contains(&fragment.conversation_id)
        }
        ArchiveReplayRecord::File(file, ..) => {
            selected_file_ids.contains(&file.id)
                || matches!(selection, crate::rewrite::ArchiveSelection::AllRetained)
        }
        ArchiveReplayRecord::ImportReceipt(_) => false,
    });
    let count = replay_archive_tail(destination, &tail)?;
    Ok(count)
}

pub(crate) fn replay_rewrite_file_links(
    source: &Cva,
    destination: &mut Cva,
    retained: &std::collections::BTreeSet<crate::MemoryId>,
) -> Result<(), CvaReconcileError> {
    for file in source.files() {
        for link in source.file_memory_links(file.id) {
            if retained.contains(&link.memory_id) {
                destination.archive.link_file_to_memory(
                    &mut destination.container,
                    link.file_id,
                    link.memory_id,
                )?;
            }
        }
    }
    Ok(())
}

pub(crate) fn read_archive_tail(
    cva: &mut Cva,
    start_chunk: usize,
) -> Result<ArchiveTail, CvaReconcileError> {
    read_archive_range(cva, start_chunk, usize::MAX)
}

/// Read a finite source interval without moving later epochs ahead of it.
pub(crate) fn read_archive_range(
    cva: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<ArchiveTail, CvaReconcileError> {
    let chunks = cva.container.chunks()?;
    let mut records = Vec::new();
    let mut file_memory_links = Vec::new();

    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = cva.container.read(*chunk)?;
        let Some(version) = crate::archive_codec::publication::version_at(*chunk, &payload)? else {
            continue;
        };
        let transaction_time_ns = cva.transaction_time_ns(version.global_version);
        let (record, applying) = cva.archive.read_publication(&mut cva.container, version)?;
        if applying.archive_version != version.archive_version {
            // Reconciliation replays semantic changes; repeated source bindings
            // must never revert a later destination mutation.
            continue;
        }
        let record = match record {
            ArchiveRecord::ImportReceipt(receipt) => ArchiveReplayRecord::ImportReceipt(receipt),
            ArchiveRecord::Node(node) => {
                let content = cva.archive.content(&mut cva.container, node.content_id)?;
                ArchiveReplayRecord::Node(IncomingTurn {
                    id: node.id,
                    conversation_id: node.conversation_id,
                    parent_id: node.parent_id,
                    role: node.role,
                    principal_id: node.principal_id,
                    timestamp_ns: node.timestamp_ns,
                    content,
                    attachments: Vec::new(),
                    project_attachments: Vec::new(),
                })
            }
            ArchiveRecord::IngestedTurn(turn) => {
                let content = cva
                    .archive
                    .content(&mut cva.container, turn.node.content_id)?;
                let mut attachments = Vec::with_capacity(turn.attachments.len());
                let mut project_attachments = Vec::new();
                for file in turn.attachments {
                    if cva.project_file_ref(file.id).is_some() {
                        project_attachments.push(file);
                    } else {
                        let bytes = cva.archive.file_bytes(&mut cva.container, file.id)?;
                        attachments.push(IncomingAttachment {
                            filename: file.filename,
                            mime_type: file.mime_type,
                            bytes,
                        });
                    }
                }
                ArchiveReplayRecord::IngestedTurn(IncomingTurn {
                    id: turn.node.id,
                    conversation_id: turn.node.conversation_id,
                    parent_id: turn.node.parent_id,
                    role: turn.node.role,
                    principal_id: turn.node.principal_id,
                    timestamp_ns: turn.node.timestamp_ns,
                    content,
                    attachments,
                    project_attachments,
                })
            }
            ArchiveRecord::Branch(branch) => ArchiveReplayRecord::Branch(branch),
            ArchiveRecord::ConversationMetadata(metadata) => {
                ArchiveReplayRecord::ConversationMetadata(metadata)
            }
            ArchiveRecord::ConversationTombstone(tombstone) => {
                ArchiveReplayRecord::ConversationTombstone(tombstone)
            }
            ArchiveRecord::Episode(episode) => ArchiveReplayRecord::Episode(episode),
            ArchiveRecord::File(file) => {
                if let Some(reference) = cva.project_file_ref(file.id) {
                    ArchiveReplayRecord::File(file, None, Some(reference))
                } else {
                    let bytes = cva.archive.file_bytes(&mut cva.container, file.id)?;
                    ArchiveReplayRecord::File(file, Some(bytes), None)
                }
            }
            ArchiveRecord::FileMemoryLink(link) => {
                file_memory_links.push(FileMemoryLinkReplay {
                    link,
                    transaction_time_ns,
                });
                continue;
            }
            ArchiveRecord::Fragment(fragment) => ArchiveReplayRecord::Fragment(fragment),
            ArchiveRecord::Content(_, _) | ArchiveRecord::Other => {
                return Err(CvaReconcileError::Archive(
                    ArchiveError::InvalidArchiveRecordVersion,
                ));
            }
        };
        records.push(ArchiveReplayEntry {
            record,
            transaction_time_ns,
        });
    }

    Ok(ArchiveTail {
        records,
        file_memory_links,
    })
}

pub(crate) fn replay_archive_tail(
    destination: &mut Cva,
    tail: &ArchiveTail,
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive_version();
    for entry in &tail.records {
        with_replayed_transaction_time(destination, entry.transaction_time_ns, |destination| {
            replay_archive_record(destination, &entry.record)
        })?;
    }
    Ok(destination.archive_version().saturating_sub(before) as usize)
}

fn replay_archive_record(
    destination: &mut Cva,
    record: &ArchiveReplayRecord,
) -> Result<(), CvaReconcileError> {
    let archive_result = match record {
        ArchiveReplayRecord::ImportReceipt(receipt) => destination
            .archive
            .put_import_receipt(&mut destination.container, receipt.clone())
            .map(|_| ()),
        ArchiveReplayRecord::Node(turn) => destination
            .append_node_with_principal(
                turn.id.clone(),
                turn.conversation_id.clone(),
                turn.parent_id.clone(),
                turn.role.clone(),
                turn.principal_id.clone(),
                turn.timestamp_ns,
                &turn.content,
            )
            .map(|_| ()),
        ArchiveReplayRecord::IngestedTurn(turn) => {
            destination.ingest_turn(turn.clone()).map(|_| ())
        }
        ArchiveReplayRecord::Branch(branch) => destination.append_branch(branch.clone()),
        ArchiveReplayRecord::ConversationMetadata(metadata) => destination
            .archive
            .put_conversation_metadata(&mut destination.container, metadata.clone())
            .map(|_| ()),
        ArchiveReplayRecord::ConversationTombstone(tombstone) => {
            if let Some(existing) = destination
                .archive
                .conversation_tombstone(&tombstone.conversation_id)
            {
                return if existing.closure == tombstone.closure {
                    Ok(())
                } else {
                    Err(CvaReconcileError::Conflict(
                        crate::CvaReconcileConflict::ConversationTombstone {
                            conversation_id: tombstone.conversation_id.clone(),
                        },
                    ))
                };
            }
            if destination
                .archive
                .discover_conversation_tombstone_closure(&tombstone.conversation_id)
                != tombstone.closure
            {
                return Err(CvaReconcileError::Conflict(
                    crate::CvaReconcileConflict::ConversationTombstone {
                        conversation_id: tombstone.conversation_id.clone(),
                    },
                ));
            }
            destination
                .tombstone_conversation(&tombstone.conversation_id)
                .map(|_| ())
        }
        ArchiveReplayRecord::Episode(episode) => destination
            .archive
            .put_episode(&mut destination.container, episode.clone())
            .map(|_| ()),
        ArchiveReplayRecord::Fragment(fragment) => destination
            .archive
            .put_fragment(&mut destination.container, fragment.clone())
            .map(|_| ()),
        ArchiveReplayRecord::File(file, Some(bytes), None) => destination
            .store_file(file.filename.clone(), file.mime_type.clone(), bytes)
            .map(|_| ()),
        ArchiveReplayRecord::File(file, None, Some(reference)) => {
            return replay_project_file(destination, file, reference.clone());
        }
        ArchiveReplayRecord::File(_, _, _) => Err(ArchiveError::InvalidArchiveRecordVersion),
    };
    archive_result.map_err(|error| archive_replay_error(destination, record, error))
}

fn replay_project_file(
    destination: &mut Cva,
    file: &StoredFile,
    reference: ProjectFileRef,
) -> Result<(), CvaReconcileError> {
    // Historical embedded FileIds can later acquire a repository binding.
    // Keep that accepted identity instead of allocating a native project FileId.
    destination
        .project_files
        .put(&mut destination.container, file.id, reference)?;
    destination
        .archive
        .register_file(&mut destination.container, file.clone())?;
    Ok(())
}

pub(crate) fn replay_file_memory_links(
    destination: &mut Cva,
    links: &[FileMemoryLinkReplay],
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive_version();
    for entry in links {
        with_replayed_transaction_time(destination, entry.transaction_time_ns, |destination| {
            destination.link_file_to_memory(entry.link.file_id, entry.link.memory_id)
        })?;
    }
    Ok(destination.archive_version().saturating_sub(before) as usize)
}
