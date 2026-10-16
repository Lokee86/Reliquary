use crate::ContainerError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EchoEventKind {
    ReasoningSummary,
    Commentary,
    ReasoningTrace,
    ToolCall,
    ToolResult,
    /// Receiver-owned acknowledgment; never model-visible execution evidence.
    ToolContextAccepted,
    ActivityStarted,
    ActivityCompleted,
    ActivityFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EchoEvent {
    pub conversation_id: String,
    pub message_id: String,
    pub sequence: u64,
    pub timestamp_ns: i64,
    pub model_round: Option<u32>,
    pub kind: EchoEventKind,
    pub correlation_id: Option<String>,
    pub name: Option<String>,
    pub content: String,
}

#[derive(Debug)]
pub enum EchoError {
    Container(ContainerError),
    InvalidRecord(&'static str),
    RecordTooLarge,
    ConflictingSequence,
}

impl fmt::Display for EchoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => error.fmt(f),
            Self::InvalidRecord(message) => write!(f, "invalid Echo record: {message}"),
            Self::RecordTooLarge => write!(f, "Echo record is too large"),
            Self::ConflictingSequence => write!(f, "Echo sequence already contains another event"),
        }
    }
}

impl std::error::Error for EchoError {}

impl From<ContainerError> for EchoError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
