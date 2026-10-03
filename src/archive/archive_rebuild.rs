use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::archive_history_codec::{decode_archive_format, decode_record_version};
use crate::archive_object_index::{ContentIndex, FragmentIndex};
use crate::archive_record_index::{BranchIndex, NodeIndex};
use crate::archive_store::hash_content;
use crate::conversation_metadata_index::ConversationMetadataIndex;
use crate::episode_index::EpisodeIndex;
use crate::file_index::FileIndex;
use crate::file_memory_link_index::FileMemoryLinkIndex;
use crate::source_attachment_index::SourceAttachmentIndex;
use crate::{Archive, ArchiveError, ArchiveRecordVersion, ObjectRef};
use std::collections::{HashMap, HashSet};

pub(crate) struct ArchiveOpenState {
    contents: ContentIndex,
    nodes: NodeIndex,
    activity: crate::archive_activity::ArchiveActivityIndex,
    branches: BranchIndex,
    conversations: ConversationMetadataIndex,
    fragments: FragmentIndex,
    episodes: EpisodeIndex,
    files: FileIndex,
    source_attachments: SourceAttachmentIndex,
    file_memory_links: FileMemoryLinkIndex,
    record_versions: Vec<ArchiveRecordVersion>,
    pending_records: HashMap<ObjectRef, ArchiveRecord>,
    versioned_records: HashSet<ObjectRef>,
    next_archive_version: u64,
    format_seen: bool,
}

impl ArchiveOpenState {
    pub(crate) fn new() -> Self {
        Self {
            contents: Default::default(),
            nodes: Default::default(),
            activity: Default::default(),
            branches: Default::default(),
            conversations: Default::default(),
            fragments: Default::default(),
            episodes: Default::default(),
            files: Default::default(),
            source_attachments: Default::default(),
            file_memory_links: Default::default(),
            record_versions: Vec::new(),
            pending_records: HashMap::new(),
            versioned_records: HashSet::new(),
            next_archive_version: 1,
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), ArchiveError> {
        if decode_archive_format(payload)? {
            if self.format_seen {
                return Err(ArchiveError::ConflictingArchiveFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(version) = decode_record_version(payload)? {
            self.ingest_version(chunk, version, latest_global_version)?;
            return Ok(());
        }
        match decode_record(payload)? {
            ArchiveRecord::Content(id, bytes) => {
                if hash_content(&bytes) != id {
                    return Err(ArchiveError::CorruptContent);
                }
                self.contents.insert(id, chunk);
            }
            record @ (ArchiveRecord::Node(_)
            | ArchiveRecord::Branch(_)
            | ArchiveRecord::ConversationMetadata(_)
            | ArchiveRecord::Fragment(_)
            | ArchiveRecord::Episode(_)
            | ArchiveRecord::File(_)
            | ArchiveRecord::IngestedTurn(_)
            | ArchiveRecord::FileMemoryLink(_)) => {
                self.pending_records.insert(chunk, record);
            }
            ArchiveRecord::Other => {}
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<Archive, ArchiveError> {
        if !self.format_seen {
            return Err(ArchiveError::MissingArchiveFormat);
        }
        Ok(Archive {
            contents: self.contents,
            nodes: self.nodes,
            activity: self.activity,
            branches: self.branches,
            conversations: self.conversations,
            fragments: self.fragments,
            episodes: self.episodes,
            files: self.files,
            source_attachments: self.source_attachments,
            file_memory_links: self.file_memory_links,
            record_versions: self.record_versions,
            next_archive_version: self.next_archive_version,
        })
    }

    fn ingest_version(
        &mut self,
        metadata_chunk: ObjectRef,
        version: ArchiveRecordVersion,
        latest_global_version: u64,
    ) -> Result<(), ArchiveError> {
        if version.archive_version != self.next_archive_version
            || version.global_version == 0
            || version.global_version > latest_global_version
            || self
                .record_versions
                .last()
                .is_some_and(|last| last.global_version >= version.global_version)
            || !version.record.precedes(metadata_chunk)
        {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }

        if let Some(record) = self.pending_records.remove(&version.record) {
            self.apply_record(record, version.archive_version)?;
            self.versioned_records.insert(version.record);
        } else if !self.versioned_records.contains(&version.record) {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }

        self.record_versions.push(version);
        self.next_archive_version = self
            .next_archive_version
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)?;
        Ok(())
    }

    fn apply_record(
        &mut self,
        record: ArchiveRecord,
        archive_version: u64,
    ) -> Result<(), ArchiveError> {
        match record {
            ArchiveRecord::Node(node) => {
                let prepared = self.activity.prepare_node(&mut self.nodes, &node)?;
                let inserted = self.nodes.insert(node)?;
                if let Some(prepared) = prepared.filter(|_| inserted) {
                    self.activity.commit_new(prepared, archive_version);
                }
                Ok(())
            }
            ArchiveRecord::Branch(branch) => {
                self.branches.put(branch);
                Ok(())
            }
            ArchiveRecord::ConversationMetadata(metadata) => {
                self.conversations.put(metadata);
                Ok(())
            }
            ArchiveRecord::Fragment(fragment) => {
                self.fragments.insert(fragment, archive_version).map(|_| ())
            }
            ArchiveRecord::Episode(episode) => self.episodes.insert(episode).map(|_| ()),
            ArchiveRecord::File(file) => self.files.insert(file).map(|_| ()),
            ArchiveRecord::IngestedTurn(turn) => {
                let conversation_id = turn.node.conversation_id.clone();
                let node_id = turn.node.id.clone();
                let file_ids = turn.attachments.iter().map(|file| file.id).collect();
                let prepared = self.activity.prepare_node(&mut self.nodes, &turn.node)?;
                let inserted = self.nodes.insert(turn.node)?;
                for file in turn.attachments {
                    self.files.insert(file)?;
                }
                self.source_attachments
                    .insert(&conversation_id, &node_id, file_ids)?;
                if let Some(prepared) = prepared.filter(|_| inserted) {
                    self.activity.commit_new(prepared, archive_version);
                }
                Ok(())
            }
            ArchiveRecord::FileMemoryLink(link) => {
                self.file_memory_links.insert(link);
                Ok(())
            }
            _ => Err(ArchiveError::InvalidArchiveRecordVersion),
        }
    }
}
