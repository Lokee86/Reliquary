use reliquary_container::ContainerError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EchoEventKind {
    ReasoningSummary,
    Commentary,
    ReasoningTrace,
    ToolCall,
    ToolResult,
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
    WrongFileKind,
    WrongOwner,
    InvalidRecord(&'static str),
    RecordTooLarge,
    ConflictingSequence,
    IndexCapacityExhausted,
}

impl fmt::Display for EchoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "Echo Container error: {error}"),
            Self::WrongFileKind => f.write_str("Echo requires a Reliquary Container"),
            Self::WrongOwner => f.write_str("Echo state belongs to another Container owner"),
            Self::InvalidRecord(reason) => write!(f, "invalid Echo record: {reason}"),
            Self::RecordTooLarge => f.write_str("Echo record is too large"),
            Self::ConflictingSequence => {
                f.write_str("Echo sequence already contains another event")
            }
            Self::IndexCapacityExhausted => f.write_str("Echo index capacity exhausted"),
        }
    }
}

impl std::error::Error for EchoError {}

impl From<ContainerError> for EchoError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
