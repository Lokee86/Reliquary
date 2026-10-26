use crate::{IncomingAttachment, IncomingTurn};
pub use reliquary_archive::InteractionRole;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionAttachment {
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTurn {
    pub message_id: String,
    pub session_id: String,
    pub parent_message_id: Option<String>,
    pub role: InteractionRole,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
    pub attachments: Vec<InteractionAttachment>,
    pub project_attachments: Vec<crate::StoredFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionSession {
    pub session_id: String,
    pub leaf_message_id: Option<String>,
    pub message_in_progress: Option<String>,
}

impl From<InteractionAttachment> for IncomingAttachment {
    fn from(value: InteractionAttachment) -> Self {
        Self {
            filename: value.filename,
            mime_type: value.mime_type,
            bytes: value.bytes,
        }
    }
}

impl From<InteractionTurn> for IncomingTurn {
    fn from(value: InteractionTurn) -> Self {
        Self {
            id: value.message_id,
            conversation_id: value.session_id,
            parent_id: value.parent_message_id,
            role: value.role.archive_role().into(),
            principal_id: value.principal_id,
            timestamp_ns: value.timestamp_ns,
            content: value.content,
            attachments: value.attachments.into_iter().map(Into::into).collect(),
            project_attachments: value.project_attachments,
        }
    }
}
