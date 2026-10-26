use crate::ConversationMetadata;
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct ConversationMetadataIndex {
    records: HashMap<String, ConversationMetadata>,
}

impl ConversationMetadataIndex {
    pub(crate) fn get(&self, conversation_id: &str) -> Option<&ConversationMetadata> {
        self.records.get(conversation_id)
    }

    pub(crate) fn put(&mut self, metadata: ConversationMetadata) {
        self.records
            .insert(metadata.conversation_id.clone(), metadata);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &ConversationMetadata> {
        self.records.values()
    }
}
