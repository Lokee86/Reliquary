use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::archive_history_codec::decode_archive_format;
use crate::archive_object_index::{ContentIndex, FragmentIndex};
use crate::archive_record_index::{BranchIndex, NodeIndex};
use crate::archive_store::hash_content;
use crate::conversation_metadata_index::ConversationMetadataIndex;
use crate::episode_index::EpisodeIndex;
use crate::file_index::FileIndex;
use crate::file_memory_link_index::FileMemoryLinkIndex;
use crate::source_attachment_index::SourceAttachmentIndex;
use crate::{Archive, ArchiveError, ArchiveRecordVersion, ObjectRef};
use std::collections::HashSet;

#[cfg(test)]
#[path = "archive_publication_replay_tests.rs"]
mod publication_tests;

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
    applying_versions: HashSet<u64>,
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
            applying_versions: HashSet::new(),
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
        if let Some(envelope) = crate::container::publication_envelope::decode(payload)? {
            self.ingest_publication_envelope(chunk, envelope, latest_global_version)?;
            return Ok(());
        }
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
        // Content is shared immutable backing. All semantic Archive records
        // are admitted only through native publication envelopes.
        if payload.starts_with(b"CVACONT1") {
            let ArchiveRecord::Content(id, bytes) = decode_record(payload)? else {
                unreachable!()
            };
            if hash_content(&bytes) != id {
                return Err(ArchiveError::CorruptContent);
            }
            self.contents.insert(id, chunk);
        }
        Ok(())
    }

    pub(crate) fn ingest_publication_envelope(
        &mut self,
        chunk: ObjectRef,
        envelope: crate::container::publication_envelope::PublicationEnvelope<'_>,
        latest_global_version: u64,
    ) -> Result<bool, ArchiveError> {
        use crate::archive_codec::publication::{self, ArchivePublication};
        let Some(publication) = publication::decode(envelope.body, envelope.global_version)? else {
            return Ok(false);
        };
        let local = publication.archive_version();
        if local != self.next_archive_version
            || envelope.global_version > latest_global_version
            || self
                .record_versions
                .last()
                .is_some_and(|last| last.global_version >= envelope.global_version)
        {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }
        let next = local
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)?;
        match publication {
            ArchivePublication::Record { record, .. } => {
                self.apply_record(record, local, envelope.global_version)?;
                self.applying_versions.insert(local);
            }
            ArchivePublication::Repeat {
                first_applying_archive_version,
                ..
            } => {
                if !self
                    .applying_versions
                    .contains(&first_applying_archive_version)
                {
                    return Err(ArchiveError::InvalidArchiveRecordVersion);
                }
            }
        }
        self.record_versions.push(ArchiveRecordVersion {
            archive_version: local,
            global_version: envelope.global_version,
            record: chunk,
        });
        self.next_archive_version = next;
        Ok(true)
    }

    pub(crate) fn finish(self) -> Result<Archive, ArchiveError> {
        if !self.format_seen {
            return Err(ArchiveError::MissingArchiveFormat);
        }
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
