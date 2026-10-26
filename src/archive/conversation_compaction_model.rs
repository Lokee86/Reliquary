use reliquary_container::ContainerError;
use std::fmt;

pub const MAX_CONVERSATION_COMPACTION_SUMMARY_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationCompaction {
    pub conversation_id: String,
    pub through_message_id: String,
    pub summary: String,
    pub generation: u64,
}

#[derive(Debug)]
pub enum ConversationCompactionError {
    Container(ContainerError),
    InvalidRecord(&'static str),
    RecordTooLarge,
    GenerationExhausted,
}

impl fmt::Display for ConversationCompactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => error.fmt(f),
            Self::InvalidRecord(message) => write!(f, "invalid conversation compaction: {message}"),
            Self::RecordTooLarge => write!(f, "conversation compaction record is too large"),
            Self::GenerationExhausted => write!(f, "conversation compaction generation exhausted"),
        }
    }
}

impl std::error::Error for ConversationCompactionError {}

impl From<ContainerError> for ConversationCompactionError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
