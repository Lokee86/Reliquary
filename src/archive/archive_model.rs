#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContentId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: String,
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content_id: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    pub id: String,
    pub conversation_id: String,
    pub leaf_node_id: String,
    pub canonical: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTurn {
    pub node_id: String,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTombstoneClosure {
    pub node_ids: Vec<String>,
    pub branch_ids: Vec<String>,
    pub fragment_ids: Vec<crate::FragmentId>,
    pub episode_ids: Vec<crate::EpisodeId>,
    pub file_ids: Vec<crate::FileId>,
    pub metadata_archive_versions: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTombstone {
    pub conversation_id: String,
    pub closure: ConversationTombstoneClosure,
    pub archive_version: u64,
    pub global_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub title: Option<String>,
    pub active: bool,
    pub leaf_node_ids: Vec<String>,
    pub turn_count: usize,
    pub latest_timestamp_ns: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveStats {
    pub content_objects: usize,
    pub nodes: usize,
    pub branches: usize,
    pub fragments: usize,
    pub episodes: usize,
    pub files: usize,
    pub source_attachments: usize,
    pub file_memory_links: usize,
}
