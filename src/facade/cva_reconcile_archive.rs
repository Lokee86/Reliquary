use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::archive_history_codec::decode_record_version;
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
        let Some(version) = decode_record_version(&payload)? else {
            continue;
        };
        let transaction_time_ns = cva.transaction_time_ns(version.global_version);
        let record_payload = cva.container.read(version.record)?;
        let record = match decode_record(&record_payload)? {
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
            return destination
                .register_project_file(
                    file.filename.clone(),
                    file.mime_type.clone(),
                    file.byte_length,
                    reference.clone(),
                )
                .map(|_| ())
                .map_err(CvaReconcileError::from);
        }
        ArchiveReplayRecord::File(_, _, _) => Err(ArchiveError::InvalidArchiveRecordVersion),
    };
    archive_result.map_err(|error| archive_replay_error(destination, record, error))
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
