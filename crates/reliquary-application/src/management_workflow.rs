use reliquary_owner_session::model::{MemoryDraft, MemoryId, MemorySourceRef, PayloadState};
use reliquary_owner_session::{RelSession, RelWriter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

pub const MAX_MANAGEMENT_PAGE_ITEMS: usize = 128;
pub const MAX_MANAGEMENT_PAGE_BYTES: usize = 256 * 1024;
const MAX_MANAGEMENT_OFFSET: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagementPage<T> {
    pub items: Vec<T>,
    pub offset: usize,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedBranch {
    pub id: String,
    pub leaf_node_id: String,
    pub canonical: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationInspection {
    pub conversation_id: String,
    pub title: Option<String>,
    pub branch: ManagedBranch,
    pub turn_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTurn {
    pub node_id: String,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemorySummary {
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub content: String,
    pub category: String,
    pub memory_type: String,
    pub lifecycle_state: String,
    pub archived: bool,
    pub payload_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryProvenance {
    pub memory_id: String,
    pub owner_id: String,
    pub principal_id: Option<String>,
    pub source_node_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagementVersions {
    pub archive: u64,
    pub memory: u64,
    pub graph: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryStatistics {
    pub memories: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticOwnerStatistics {
    pub memories: MemoryStatistics,
    pub conversations: usize,
    pub turns: u64,
    pub versions: ManagementVersions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelManagementHealth {
    pub conversations: usize,
    pub memories: usize,
    pub versions: ManagementVersions,
    pub admitted: bool,
}

#[derive(Debug)]
pub enum ManagementError {
    InvalidPage,
    PageBytesExceeded,
    NotFound,
    InvalidInput(&'static str),
    Owner(String),
}

impl fmt::Display for ManagementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPage => f.write_str("management page is outside supported bounds"),
            Self::PageBytesExceeded => f.write_str("management page byte bound exceeded"),
            Self::NotFound => f.write_str("management item was not found"),
            Self::InvalidInput(reason) => write!(f, "invalid management input: {reason}"),
            Self::Owner(reason) => write!(f, "owner management failed: {reason}"),
        }
    }
}
impl std::error::Error for ManagementError {}

pub fn management_conversations(
    owner: &RelSession,
    offset: usize,
    limit: usize,
) -> Result<ManagementPage<ConversationInspection>, ManagementError> {
    validate_page(offset, limit)?;
    let all = owner
        .conversation_summaries()
        .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let total = all.len();
    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|value| ConversationInspection {
            conversation_id: value.conversation_id,
            title: value.title,
            branch: ManagedBranch {
                id: value.branch_id,
                leaf_node_id: value.leaf_node_id,
                canonical: value.canonical,
            },
            turn_count: value.turn_count,
        })
        .collect::<Vec<_>>();
    bounded_page(items, offset, total)
}

pub fn management_transcript(
    owner: &RelSession,
    conversation_id: &str,
    leaf_node_id: &str,
    offset: usize,
    limit: usize,
) -> Result<ManagementPage<TranscriptTurn>, ManagementError> {
    validate_page(offset, limit)?;
    let turns = owner
        .transcript(conversation_id, leaf_node_id)
        .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let total = turns.len();
    let items = turns
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|turn| TranscriptTurn {
            node_id: turn.node_id,
            role: turn.role,
            principal_id: turn.principal_id,
            timestamp_ns: turn.timestamp_ns,
            content: turn.content,
        })
        .collect::<Vec<_>>();
    bounded_page(items, offset, total)
}

pub fn set_management_title(
    writer: &mut RelWriter,
    conversation_id: &str,
    title: &str,
) -> Result<bool, ManagementError> {
    if conversation_id.trim().is_empty() || title.trim().is_empty() || title.len() > 4096 {
        return Err(ManagementError::InvalidInput("conversation title"));
    }
    writer
        .set_conversation_title(conversation_id, title)
        .map_err(|error| ManagementError::Owner(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn create_direct_memory(
    writer: &mut RelWriter,
    principal: &str,
    title: String,
    content: String,
    category: String,
    memory_type: String,
    created_at_ns: i64,
) -> Result<MemorySummary, ManagementError> {
    if principal.trim().is_empty()
        || title.trim().is_empty()
        || content.trim().is_empty()
        || category.trim().is_empty()
        || memory_type.trim().is_empty()
        || created_at_ns < 0
    {
        return Err(ManagementError::InvalidInput("direct Memory"));
    }
    let owner_id = hex(&writer.owner_id());
    let mut digest = Sha256::new();
    digest.update(b"reliquary:management-memory:v1\0");
    for value in [
        principal.as_bytes(),
        title.as_bytes(),
        content.as_bytes(),
        category.as_bytes(),
        memory_type.as_bytes(),
    ] {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value);
    }
    digest.update(created_at_ns.to_le_bytes());
    let mutation_id = format!("management:{}", hex(&digest.finalize()));
    let source_node_id = format!("management:{mutation_id}");
    let draft = MemoryDraft {
        title,
        content,
        category,
        memory_type,
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: Some(source_node_id.clone()),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id,
            principal_id: Some(principal.to_owned()),
            source_episode_id: [0; 32],
            source_node_id,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(created_at_ns),
        temporal_inference: None,
        mutation_id,
        created_at_ns,
        updated_at_ns: created_at_ns,
    };
    let revision = writer
        .publish_direct_memory(principal, draft)
        .map_err(|error| ManagementError::Owner(error.to_string()))?;
    summary(revision)
}

pub fn management_memory(
    owner: &RelSession,
    memory_id: MemoryId,
) -> Result<MemorySummary, ManagementError> {
    summary(
        owner
            .current_memory(memory_id)
            .map_err(|error| ManagementError::Owner(error.to_string()))?,
    )
}

pub fn management_memories(
    owner: &RelSession,
    offset: usize,
    limit: usize,
) -> Result<ManagementPage<MemorySummary>, ManagementError> {
    validate_page(offset, limit)?;
    let mut ids = Vec::new();
    let mut after = None;
    loop {
        let (_, page) = owner
            .memory_ids_page(after, MAX_MANAGEMENT_PAGE_ITEMS)
            .map_err(|error| ManagementError::Owner(error.to_string()))?;
        if page.is_empty() {
            break;
        }
        after = page.last().copied();
        ids.extend(page);
        if ids.len() > MAX_MANAGEMENT_OFFSET + MAX_MANAGEMENT_PAGE_ITEMS {
            break;
        }
    }
    let total = ids.len();
    let items = ids
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|id| management_memory(owner, id))
        .collect::<Result<Vec<_>, _>>()?;
    bounded_page(items, offset, total)
}

pub fn management_provenance(
    owner: &RelSession,
    memory_id: MemoryId,
) -> Result<MemoryProvenance, ManagementError> {
    let revision = owner
        .current_memory(memory_id)
        .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let source = revision
        .record
        .source_ref
        .ok_or(ManagementError::NotFound)?;
    Ok(MemoryProvenance {
        memory_id: hex(&memory_id.0),
        owner_id: source.owner_id,
        principal_id: source.principal_id,
        source_node_id: source.source_node_id,
    })
}

pub fn management_statistics(
    owner: &RelSession,
) -> Result<SemanticOwnerStatistics, ManagementError> {
    let memories = management_memories(owner, 0, MAX_MANAGEMENT_PAGE_ITEMS)?.total;
    let conversations = owner
        .conversation_summaries()
        .map_err(|error| ManagementError::Owner(error.to_string()))?
        .len();
    Ok(SemanticOwnerStatistics {
        memories: MemoryStatistics { memories },
        conversations,
        turns: owner.rel_turn_count(),
        versions: versions(owner),
    })
}

pub fn management_health(owner: &RelSession) -> Result<RelManagementHealth, ManagementError> {
    let statistics = management_statistics(owner)?;
    Ok(RelManagementHealth {
        conversations: statistics.conversations,
        memories: statistics.memories.memories,
        versions: statistics.versions,
        admitted: true,
    })
}

fn versions(owner: &RelSession) -> ManagementVersions {
    ManagementVersions {
        archive: owner.archive_version(),
        memory: owner.memory_version(),
        graph: owner.graph_version(),
    }
}

fn summary(
    revision: reliquary_owner_session::model::MemoryRevision,
) -> Result<MemorySummary, ManagementError> {
    let (title, content, payload_available) = match revision.payload {
        PayloadState::Available(body) => (body.title, body.content, true),
        PayloadState::Absent => (String::new(), String::new(), false),
    };
    Ok(MemorySummary {
        id: hex(&revision.record.id.0),
        revision: revision.record.revision,
        title,
        content,
        category: revision.record.category,
        memory_type: revision.record.memory_type,
        lifecycle_state: revision.record.lifecycle_state,
        archived: revision.record.archived,
        payload_available,
    })
}

fn validate_page(offset: usize, limit: usize) -> Result<(), ManagementError> {
    if offset > MAX_MANAGEMENT_OFFSET || !(1..=MAX_MANAGEMENT_PAGE_ITEMS).contains(&limit) {
        Err(ManagementError::InvalidPage)
    } else {
        Ok(())
    }
}

fn bounded_page<T: Serialize>(
    items: Vec<T>,
    offset: usize,
    total: usize,
) -> Result<ManagementPage<T>, ManagementError> {
    let bytes =
        serde_json::to_vec(&items).map_err(|error| ManagementError::Owner(error.to_string()))?;
    if bytes.len() > MAX_MANAGEMENT_PAGE_BYTES {
        return Err(ManagementError::PageBytesExceeded);
    }
    let next = offset.checked_add(items.len()).filter(|next| *next < total);
    Ok(ManagementPage {
        items,
        offset,
        total,
        next_offset: next,
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}
