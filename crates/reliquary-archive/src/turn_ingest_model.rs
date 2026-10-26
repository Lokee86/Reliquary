use crate::{Node, StoredFile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncomingAttachment {
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncomingTurn {
    pub id: String,
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
    pub attachments: Vec<IncomingAttachment>,
    pub project_attachments: Vec<StoredFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestedTurn {
    pub node: Node,
    pub attachments: Vec<StoredFile>,
}
