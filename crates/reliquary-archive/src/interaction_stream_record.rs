use crate::StoredFile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionRole {
    User,
    Agent,
}

impl InteractionRole {
    pub fn archive_role(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "assistant",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionStreamStatus {
    Streaming,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionStreamRecord {
    pub message_id: String,
    pub session_id: String,
    pub parent_message_id: Option<String>,
    pub role: InteractionRole,
    pub timestamp_ns: i64,
    pub content: String,
    pub status: InteractionStreamStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionTurnStatus {
    Complete,
    Streaming,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInteractionTurn {
    pub message_id: String,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
    pub attachments: Vec<StoredFile>,
    pub status: InteractionTurnStatus,
}
