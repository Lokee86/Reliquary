//! Archive lifecycle access through the existing mounted REL owner.
use super::{
    LifecycleError, LifecycleState, LifecycleTarget, ReliquaryOwner, TombstoneCoordinates,
};
use crate::{
    ArchiveError, ConversationSummary, ConversationTombstone, FileId, FileMemoryLink, Fragment,
    MemoryId, ResolvedTurn, StoredFile,
};

impl ReliquaryOwner {
    pub(crate) fn archive_read_view(&self) -> crate::archive::ArchiveReadView<'_> {
        crate::archive::ArchiveReadView::new(&self.archive, &self.container)
    }

    pub(crate) fn native_historical_embedding_nodes(&self) -> impl Iterator<Item = &crate::Node> {
        self.archive.nodes.iter()
    }

    pub(crate) fn native_historical_embedding_fragments(&self) -> impl Iterator<Item = &Fragment> {
        self.archive.fragments.iter()
    }

    pub(crate) fn native_historical_embedding_node_count(&self) -> usize {
        self.archive.nodes.len()
    }

    pub(crate) fn native_historical_embedding_fragment_count(&self) -> usize {
        self.archive.fragments.len()
    }

    pub(crate) fn native_historical_embedding_fragment_text(
        &mut self,
        fragment: &Fragment,
        max_bytes: usize,
    ) -> Result<String, ArchiveError> {
        self.archive
            .fragment_text_bounded(&mut self.container, fragment, max_bytes)
    }

    pub(crate) fn native_required_node_timestamp(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<i64, ArchiveError> {
        self.archive
            .require_node(conversation_id, node_id, ArchiveError::MissingLeaf)
            .map(|node| node.timestamp_ns)
    }

    pub(crate) fn native_activity_cut_at_archive_version(
        &self,
        version: u64,
    ) -> Result<u64, ArchiveError> {
        self.archive.activity_cut_at_archive_version(version)
    }

    pub(crate) fn native_activity_position_for_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Option<u64> {
        self.archive
            .activity_position_for_turn(conversation_id, node_id)
    }

    pub(crate) fn native_file_memory_links(&self, file_id: FileId) -> Vec<FileMemoryLink> {
        self.archive.file_memory_links(file_id)
    }

    pub(crate) fn native_files_for_memory(&self, memory_id: MemoryId) -> Vec<StoredFile> {
        self.archive.files_for_memory(memory_id)
    }

    pub(crate) fn native_historical_files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Vec<StoredFile> {
        self.archive
            .historical_files_for_source(conversation_id, node_id)
    }

    pub(crate) fn native_conversation_summaries(&self) -> Vec<ConversationSummary> {
        self.archive.conversation_summaries()
    }

    pub(crate) fn native_historical_conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.archive.historical_conversation_turns(
            &mut self.container,
            conversation_id,
            leaf_node_id,
        )
    }

    pub(crate) fn native_conversation_turn_page(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
    ) -> Result<(Vec<ResolvedTurn>, Option<String>), ArchiveError> {
        self.archive.conversation_turn_page(
            &mut self.container,
            conversation_id,
            end_node_id,
            limit,
        )
    }

    pub(crate) fn native_tombstone_conversation(
        &mut self,
        conversation_id: &str,
    ) -> Result<bool, ArchiveError> {
        self.archive
            .tombstone_conversation(&mut self.container, conversation_id)
    }

    pub(crate) fn native_conversation_tombstone(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationTombstone> {
        self.archive.conversation_tombstone(conversation_id)
    }

    pub(crate) fn native_conversation_lifecycle_state(
        &self,
        conversation_id: &str,
    ) -> Result<LifecycleState, LifecycleError> {
        if let Some(tombstone) = self.archive.conversation_tombstone(conversation_id) {
            return Ok(LifecycleState::Tombstoned(TombstoneCoordinates {
                global_version: Some(tombstone.global_version),
                domain_version: Some(tombstone.archive_version),
            }));
        }
        if self.archive.has_historical_conversation(conversation_id) {
            return Ok(LifecycleState::Active);
        }
        Err(LifecycleError::UnknownTarget(
            LifecycleTarget::Conversation(conversation_id.to_owned()),
        ))
    }
}
