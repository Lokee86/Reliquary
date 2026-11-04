use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryBodyId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRevisionId {
    pub memory_id: MemoryId,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySourceRef {
    pub owner_id: String,
    pub principal_id: Option<String>,
    pub source_episode_id: [u8; 32],
    pub source_node_id: String,
    pub content_source_conversation_id: Option<String>,
    pub content_source_node_id: Option<String>,
    pub grounding_source_conversation_id: Option<String>,
    pub grounding_source_node_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalIndicationKind {
    Explicit,
    Calendar,
    Relative,
    Boundary,
    Recurrence,
    Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalInferenceResolution {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: TemporalIndicationKind,
    pub evidence: String,
    pub canonical_expression: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalInference {
    pub model: String,
    pub contract_version: String,
    pub resolutions: Vec<TemporalInferenceResolution>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTemporalInference {
    pub body_id: MemoryBodyId,
    pub source_time_ns: Option<i64>,
    pub inference: TemporalInference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryDraft {
    pub title: String,
    pub content: String,
    pub category: String,
    pub memory_type: String,
    pub authority_kind: String,
    pub temporal_status: String,
    pub scope: String,
    pub lifecycle_state: String,
    pub archived: bool,
    pub parent_id: Option<MemoryId>,
    pub superseded_by: Option<MemoryId>,
    pub source_node_id: Option<String>,
    pub content_source_conversation_id: Option<String>,
    pub content_source_node_id: Option<String>,
    pub grounding_source_conversation_id: Option<String>,
    pub grounding_source_node_id: Option<String>,
    pub source_episode_id: Option<[u8; 32]>,
    pub source_ref: Option<MemorySourceRef>,
    pub source_time_ns: Option<i64>,
    pub temporal_inference: Option<MemoryTemporalInference>,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryBody {
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRecord {
    pub id: MemoryId,
    pub revision: u64,
    pub body_id: MemoryBodyId,
    pub category: String,
    pub memory_type: String,
    pub authority_kind: String,
    pub temporal_status: String,
    pub scope: String,
    pub lifecycle_state: String,
    pub archived: bool,
    pub parent_id: Option<MemoryId>,
    pub superseded_by: Option<MemoryId>,
    pub source_node_id: Option<String>,
    pub content_source_conversation_id: Option<String>,
    pub content_source_node_id: Option<String>,
    pub grounding_source_conversation_id: Option<String>,
    pub grounding_source_node_id: Option<String>,
    pub source_episode_id: Option<[u8; 32]>,
    pub source_ref: Option<MemorySourceRef>,
    pub source_time_ns: Option<i64>,
    pub temporal_inference: Option<MemoryTemporalInference>,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
    pub memory_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PayloadState {
    Available(MemoryBody),
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRevision {
    pub record: MemoryRecord,
    pub payload: PayloadState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTombstone {
    pub revision: u64,
    pub memory_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryHistory {
    pub revisions: Vec<MemoryRevision>,
    pub tombstone: Option<TerminalTombstone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishResult {
    pub revision: MemoryRevision,
    pub published: bool,
}

pub fn memory_id(mutation_id: &str) -> MemoryId {
    let mut hash = Sha256::new();
    hash.update(b"continuity-memory-id\0");
    hash.update((mutation_id.len() as u64).to_le_bytes());
    hash.update(mutation_id.as_bytes());
    MemoryId(hash.finalize().into())
}

pub fn memory_body_bytes(title: &str, content: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + title.len() + content.len());
    out.extend_from_slice(&(title.len() as u64).to_le_bytes());
    out.extend_from_slice(title.as_bytes());
    out.extend_from_slice(&(content.len() as u64).to_le_bytes());
    out.extend_from_slice(content.as_bytes());
    out
}

pub fn memory_body_id(title: &str, content: &str) -> MemoryBodyId {
    MemoryBodyId(Sha256::digest(memory_body_bytes(title, content)).into())
}
