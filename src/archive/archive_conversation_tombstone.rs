use crate::archive_codec::encode_conversation_tombstone;
use crate::{
    Archive, ArchiveError, Container, ConversationTombstone, ConversationTombstoneClosure,
};

impl Archive {
    pub(crate) fn tombstone_conversation(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
    ) -> Result<bool, ArchiveError> {
        crate::archive_store::validate_text(conversation_id, "conversation id")?;
        if self.is_conversation_tombstoned(conversation_id) {
            return Ok(false);
        }
        if !self.has_historical_conversation(conversation_id) {
            return Err(ArchiveError::MissingConversation);
        }
        let archive_version = self.next_archive_version;
        let global_version = container
            .latest_version()
            .checked_add(1)
            .ok_or(ArchiveError::ArchiveVersionExhausted)?;
        let tombstone = ConversationTombstone {
            conversation_id: conversation_id.to_owned(),
            closure: self.discover_conversation_tombstone_closure(conversation_id),
            archive_version,
            global_version,
        };
        let record = container.append(&encode_conversation_tombstone(&tombstone)?)?;
        let published = self.publish_record(container, record)?;
        if published.archive_version != archive_version
            || published.global_version != global_version
        {
            return Err(ArchiveError::InvalidArchiveRecordVersion);
        }
        self.conversation_tombstones
            .insert(conversation_id.to_owned(), tombstone);
        Ok(true)
    }

    pub fn conversation_tombstone(&self, conversation_id: &str) -> Option<&ConversationTombstone> {
        self.conversation_tombstones.get(conversation_id)
    }

    pub fn is_conversation_tombstoned(&self, conversation_id: &str) -> bool {
        self.conversation_tombstones.contains_key(conversation_id)
    }

    pub(crate) fn discover_conversation_tombstone_closure(
        &self,
        conversation_id: &str,
    ) -> ConversationTombstoneClosure {
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
        let mut file_ids = self
            .source_attachments
            .conversation_file_ids(conversation_id);
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
        node_ids.sort();
        branch_ids.sort();
        file_ids.sort_by_key(|id| id.0);
        fragment_ids.sort_by_key(|id| id.0);
        episode_ids.sort_by_key(|id| id.0);
        file_ids.dedup();
        ConversationTombstoneClosure {
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

impl crate::Cva {
    pub fn tombstone_conversation(&mut self, conversation_id: &str) -> Result<bool, ArchiveError> {
        self.archive
            .tombstone_conversation(&mut self.container, conversation_id)
    }

    pub fn conversation_tombstone(&self, conversation_id: &str) -> Option<&ConversationTombstone> {
        self.archive.conversation_tombstone(conversation_id)
    }
}
