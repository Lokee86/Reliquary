//! Ordered local Archive replay facts. These values carry no facade or coordinator.
use crate::archive_codec::ArchiveRecord;
use crate::{
    ArchiveError, Branch, ConversationMetadata, Episode, FileMemoryLink, Fragment,
    IncomingAttachment, IncomingTurn, ProjectFileRef, StoredFile,
};

pub(crate) struct ArchiveReplaySource<'a> {
    pub(crate) container: &'a crate::Container,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) project_files: &'a crate::project_file_binding_store::ProjectFileStore,
}
impl ArchiveReplaySource<'_> {
    pub(crate) fn project_file_ref(&self, id: crate::FileId) -> Option<ProjectFileRef> {
        self.project_files.get(id).cloned()
    }
}

pub(crate) enum ArchiveReplayRecord {
    ImportReceipt(crate::archive_import_model::ImportReceipt),
    Node(IncomingTurn),
    IngestedTurn(IncomingTurn),
    Branch(Branch),
    ConversationMetadata(ConversationMetadata),
    ConversationTombstone(crate::ConversationTombstone),
    Episode(Episode),
    Fragment(Fragment),
    File(StoredFile, Option<Vec<u8>>, Option<ProjectFileRef>),
}

pub(crate) struct ArchiveReplayEntry {
    pub(crate) record: ArchiveReplayRecord,
    pub(crate) transaction_time_ns: Option<i64>,
}

pub(crate) struct FileMemoryLinkReplay {
    pub(crate) link: FileMemoryLink,
    pub(crate) transaction_time_ns: Option<i64>,
}

pub(crate) struct ArchiveTail {
    pub(crate) records: Vec<ArchiveReplayEntry>,
    pub(crate) file_memory_links: Vec<FileMemoryLinkReplay>,
}

pub(crate) fn read_archive_range(
    source: ArchiveReplaySource<'_>,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<ArchiveTail, ArchiveError> {
    let chunks = source.container.chunk_refs()?;
    let mut records = Vec::new();
    let mut file_memory_links = Vec::new();

    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = source.container.read(*chunk)?;
        let Some(version) = crate::archive_codec::publication::version_at(*chunk, &payload)? else {
            continue;
        };
        let transaction_time_ns = source.container.transaction_time_ns(version.global_version);
        let (record, applying) = source.archive.read_publication(source.container, version)?;
        if applying.archive_version != version.archive_version {
            // Reconciliation replays semantic changes; repeated source bindings
            // must never revert a later destination mutation.
            continue;
        }
        let record = match record {
            ArchiveRecord::ImportReceipt(receipt) => ArchiveReplayRecord::ImportReceipt(receipt),
            ArchiveRecord::Node(node) => {
                let content = source.archive.content(source.container, node.content_id)?;
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
                let content = source
                    .archive
                    .content(source.container, turn.node.content_id)?;
                let mut attachments = Vec::with_capacity(turn.attachments.len());
                let mut project_attachments = Vec::new();
                for file in turn.attachments {
                    if source.project_file_ref(file.id).is_some() {
                        project_attachments.push(file);
                    } else {
                        let bytes = source.archive.file_bytes(source.container, file.id)?;
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
                if let Some(reference) = source.project_file_ref(file.id) {
                    ArchiveReplayRecord::File(file, None, Some(reference))
                } else {
                    let bytes = source.archive.file_bytes(source.container, file.id)?;
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
                return Err(ArchiveError::InvalidArchiveRecordVersion);
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

/// Local Archive publishers used after a higher operation has authorized replay.
pub(crate) struct ArchiveReplayWriter<'a> {
    pub(crate) container: &'a mut crate::Container,
    pub(crate) archive: &'a mut crate::Archive,
    pub(crate) project_files: &'a mut crate::project_file_binding_store::ProjectFileStore,
}

pub(crate) enum ArchiveReplayWriteError {
    Archive(ArchiveError),
    Project(crate::project_file_binding_store::ProjectFileStoreError),
}
impl From<ArchiveError> for ArchiveReplayWriteError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl ArchiveReplayWriter<'_> {
    /// Preserve the original one-record transaction-time override, including
    /// clearing it on refusal. No caller callback or semantic manager is held.
    pub(crate) fn replay(
        &mut self,
        entry: &ArchiveReplayEntry,
    ) -> Result<(), ArchiveReplayWriteError> {
        self.container
            .set_next_transaction_time_override(entry.transaction_time_ns);
        let result = self.replay_record(&entry.record);
        self.container.clear_next_transaction_time_override();
        result
    }
    pub(crate) fn project_file(
        &mut self,
        file: &StoredFile,
        reference: ProjectFileRef,
    ) -> Result<(), ArchiveReplayWriteError> {
        self.project_files
            .put(self.container, file.id, reference)
            .map_err(ArchiveReplayWriteError::Project)?;
        self.archive.register_file(self.container, file.clone())?;
        Ok(())
    }
    fn replay_record(
        &mut self,
        record: &ArchiveReplayRecord,
    ) -> Result<(), ArchiveReplayWriteError> {
        match record {
            ArchiveReplayRecord::ImportReceipt(receipt) => {
                self.archive
                    .put_import_receipt(self.container, receipt.clone())?;
            }
            ArchiveReplayRecord::Node(turn) => {
                self.archive.append_node(
                    self.container,
                    turn.id.clone(),
                    turn.conversation_id.clone(),
                    turn.parent_id.clone(),
                    turn.role.clone(),
                    turn.principal_id.clone(),
                    turn.timestamp_ns,
                    &turn.content,
                )?;
            }
            ArchiveReplayRecord::IngestedTurn(turn) => {
                self.archive.ingest_turn(
                    self.container,
                    self.project_files.accepted_facts(),
                    turn.clone(),
                )?;
            }
            ArchiveReplayRecord::Branch(branch) => {
                self.archive.append_branch(self.container, branch.clone())?;
            }
            ArchiveReplayRecord::ConversationMetadata(metadata) => {
                self.archive
                    .put_conversation_metadata(self.container, metadata.clone())?;
            }
            ArchiveReplayRecord::ConversationTombstone(tombstone) => {
                if let Some(existing) = self
                    .archive
                    .conversation_tombstone(&tombstone.conversation_id)
                {
                    return if existing.closure == tombstone.closure {
                        Ok(())
                    } else {
                        Err(ArchiveError::InvalidArchiveRecordVersion.into())
                    };
                }
                if self
                    .archive
                    .discover_conversation_tombstone_closure(&tombstone.conversation_id)
                    != tombstone.closure
                {
                    return Err(ArchiveError::InvalidArchiveRecordVersion.into());
                }
                self.archive
                    .tombstone_conversation(self.container, &tombstone.conversation_id)?;
            }
            ArchiveReplayRecord::Episode(episode) => {
                self.archive.put_episode(self.container, episode.clone())?;
            }
            ArchiveReplayRecord::Fragment(fragment) => {
                self.archive
                    .put_fragment(self.container, fragment.clone())?;
            }
            ArchiveReplayRecord::File(file, Some(bytes), None) => {
                self.archive.store_file(
                    self.container,
                    file.filename.clone(),
                    file.mime_type.clone(),
                    bytes,
                )?;
            }
            ArchiveReplayRecord::File(file, None, Some(reference)) => {
                self.project_file(file, reference.clone())?;
            }
            ArchiveReplayRecord::File(..) => {
                return Err(ArchiveError::InvalidArchiveRecordVersion.into());
            }
        }
        Ok(())
    }
}
