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
    import_receipts: std::collections::BTreeMap<String, crate::imports::ImportReceipt>,
    contents: ContentIndex,
    nodes: NodeIndex,
    activity: crate::archive_activity::ArchiveActivityIndex,
    branches: BranchIndex,
    conversations: ConversationMetadataIndex,
    conversation_metadata_versions: std::collections::BTreeMap<String, Vec<u64>>,
    conversation_tombstones: std::collections::BTreeMap<String, crate::ConversationTombstone>,
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
    prune: crate::archive::history_prune::ArchivePruneState,
}

impl ArchiveOpenState {
    pub(crate) fn new() -> Self {
        Self {
            import_receipts: Default::default(),
            contents: Default::default(),
            nodes: Default::default(),
            activity: Default::default(),
            branches: Default::default(),
            conversations: Default::default(),
            conversation_metadata_versions: Default::default(),
            conversation_tombstones: Default::default(),
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
            prune: Default::default(),
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), ArchiveError> {
        if self.prune.ingest(payload)? {
            return Ok(());
        }
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
            | ArchiveRecord::ConversationTombstone(_)
            | ArchiveRecord::Fragment(_)
            | ArchiveRecord::Episode(_)
            | ArchiveRecord::File(_)
            | ArchiveRecord::IngestedTurn(_)
            | ArchiveRecord::ImportReceipt(_)
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
        self.prune.validate_markers()?;
        Ok(Archive {
            prune: self.prune,
            import_receipts: self.import_receipts,
            contents: self.contents,
            nodes: self.nodes,
            activity: self.activity,
            branches: self.branches,
            conversations: self.conversations,
            conversation_metadata_versions: self.conversation_metadata_versions,
            conversation_tombstones: self.conversation_tombstones,
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
            self.apply_record(record, version.archive_version, version.global_version)?;
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
        global_version: u64,
    ) -> Result<(), ArchiveError> {
        match record {
            ArchiveRecord::ImportReceipt(receipt) => {
                if let Some(existing) = self
                    .import_receipts
                    .insert(receipt.import_key.clone(), receipt.clone())
                    && existing != receipt
                {
                    return Err(ArchiveError::CorruptRecord("conflicting import receipt"));
                }
                Ok(())
            }
            ArchiveRecord::Node(node) => {
                if self
                    .conversation_tombstones
                    .contains_key(&node.conversation_id)
                {
                    return Err(ArchiveError::TombstonedConversation);
                }
                let prepared = self.activity.prepare_node(&mut self.nodes, &node)?;
                let inserted = self.nodes.insert(node)?;
                if let Some(prepared) = prepared.filter(|_| inserted) {
                    self.activity.commit_new(prepared, archive_version);
                }
                Ok(())
            }
            ArchiveRecord::Branch(branch) => {
                if self
                    .conversation_tombstones
                    .contains_key(&branch.conversation_id)
                {
                    return Err(ArchiveError::TombstonedConversation);
                }
                self.branches.put(branch);
                Ok(())
            }
            ArchiveRecord::ConversationMetadata(metadata) => {
                if self
                    .conversation_tombstones
                    .contains_key(&metadata.conversation_id)
                {
                    return Err(ArchiveError::TombstonedConversation);
                }
                self.conversation_metadata_versions
                    .entry(metadata.conversation_id.clone())
                    .or_default()
                    .push(archive_version);
                self.conversations.put(metadata);
                Ok(())
            }
            ArchiveRecord::ConversationTombstone(tombstone) => {
                if tombstone.archive_version != archive_version
                    || tombstone.global_version != global_version
                    || !self
                        .nodes
                        .iter()
                        .any(|node| node.conversation_id == tombstone.conversation_id)
                    || self.discover_conversation_tombstone_closure(&tombstone.conversation_id)
                        != tombstone.closure
                {
                    return Err(ArchiveError::ConflictingConversationTombstone);
                }
                if let Some(existing) = self.conversation_tombstones.get(&tombstone.conversation_id)
                    && existing.closure != tombstone.closure
                {
                    return Err(ArchiveError::ConflictingConversationTombstone);
                }
                self.conversation_tombstones
                    .insert(tombstone.conversation_id.clone(), tombstone);
                Ok(())
            }
            ArchiveRecord::Fragment(fragment) => {
                if self
                    .conversation_tombstones
                    .contains_key(&fragment.conversation_id)
                {
                    return Err(ArchiveError::TombstonedConversation);
                }
                self.fragments.insert(fragment, archive_version).map(|_| ())
            }
            ArchiveRecord::Episode(episode) => {
                if self
                    .conversation_tombstones
                    .contains_key(&episode.conversation_id)
                {
                    return Err(ArchiveError::TombstonedConversation);
                }
                self.episodes.insert(episode).map(|_| ())
            }
            ArchiveRecord::File(file) => self.files.insert(file).map(|_| ()),
            ArchiveRecord::IngestedTurn(turn) => {
                let conversation_id = turn.node.conversation_id.clone();
                if self.conversation_tombstones.contains_key(&conversation_id) {
                    return Err(ArchiveError::TombstonedConversation);
                }
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

    fn discover_conversation_tombstone_closure(
        &self,
        conversation_id: &str,
    ) -> crate::ConversationTombstoneClosure {
        let mut node_ids = self
            .nodes
            .iter()
            .filter(|node| node.conversation_id == conversation_id)
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let mut branch_ids = self
            .branches
            .iter()
            .filter(|branch| branch.conversation_id == conversation_id)
            .map(|branch| branch.id.clone())
            .collect::<Vec<_>>();
        let mut fragment_ids = self
            .fragments
            .iter()
            .filter(|fragment| fragment.conversation_id == conversation_id)
            .map(|fragment| fragment.id)
            .collect::<Vec<_>>();
        let mut episode_ids = self
            .episodes
            .for_conversation(conversation_id)
            .into_iter()
            .map(|episode| episode.id)
            .collect::<Vec<_>>();
        let mut file_ids = self
            .source_attachments
            .conversation_file_ids(conversation_id);
        node_ids.sort();
        branch_ids.sort();
        fragment_ids.sort_by_key(|id| id.0);
        episode_ids.sort_by_key(|id| id.0);
        file_ids.sort_by_key(|id| id.0);
        file_ids.dedup();
        crate::ConversationTombstoneClosure {
            node_ids,
            branch_ids,
            fragment_ids,
            episode_ids,
            file_ids,
            metadata_archive_versions: self
                .conversation_metadata_versions
                .get(conversation_id)
                .cloned()
                .unwrap_or_default(),
        }
    }
}
