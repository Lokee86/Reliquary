use crate::archive_store::validate_text;
use crate::conversation_metadata_codec::encode_conversation_metadata;
use crate::{Archive, ArchiveError, Container, ConversationMetadata, ConversationPolicy};

impl Archive {
    pub(crate) fn put_conversation_metadata(
        &mut self,
        container: &mut Container,
        metadata: ConversationMetadata,
    ) -> Result<bool, ArchiveError> {
        validate_text(&metadata.conversation_id, "conversation id")?;
        self.require_current_conversation_id(&metadata.conversation_id)?;
        if !self.has_conversation(&metadata.conversation_id) {
            return Err(ArchiveError::MissingConversation);
        }
        if let Some(title) = metadata.title.as_deref() {
            validate_text(title, "conversation title")?;
        }
        if self.conversations.get(&metadata.conversation_id) == Some(&metadata) {
            return Ok(false);
        }
        let record = container.append(&encode_conversation_metadata(&metadata)?)?;
        let version = self.publish_record(container, record)?;
        self.conversation_metadata_versions
            .entry(metadata.conversation_id.clone())
            .or_default()
            .push(version.archive_version);
        self.conversations.put(metadata);
        Ok(true)
    }

    /// Absence of metadata uses the legacy default, including a new conversation.
    pub fn conversation_policy(&self, conversation_id: &str) -> (ConversationPolicy, u64) {
        if self.is_conversation_tombstoned(conversation_id) {
            return (ConversationPolicy::default(), 1);
        }
        self.conversations
            .get(conversation_id)
            .map(|metadata| (metadata.policy, metadata.policy_revision))
            .unwrap_or((ConversationPolicy::default(), 1))
    }

    pub(crate) fn set_conversation_policy(
        &mut self,
        container: &mut Container,
        conversation_id: &str,
        policy: ConversationPolicy,
    ) -> Result<bool, ArchiveError> {
        self.require_current_conversation_id(conversation_id)?;
        if !self.has_conversation(conversation_id) {
            return Err(ArchiveError::MissingConversation);
        }
        let mut metadata = self
            .conversations
            .get(conversation_id)
            .cloned()
            .unwrap_or_else(|| ConversationMetadata::new(conversation_id));
        if metadata.policy == policy {
            return Ok(false);
        }
        metadata.policy_revision = metadata
            .policy_revision
            .checked_add(1)
            .ok_or(ArchiveError::FieldTooLarge)?;
        metadata.policy = policy;
        self.put_conversation_metadata(container, metadata)
    }

    pub fn conversation_metadata(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        if self.is_conversation_tombstoned(conversation_id) {
            return None;
        }
        self.historical_conversation_metadata(conversation_id)
    }

    pub fn historical_conversation_metadata(
        &self,
        conversation_id: &str,
    ) -> Option<&ConversationMetadata> {
        self.conversations.get(conversation_id)
    }
}
