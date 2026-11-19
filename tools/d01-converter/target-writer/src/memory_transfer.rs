use crate::transfer::{decimal, decode_hex};
use reliquary_memory_core::{
    MemoryBody, MemoryBodyId, MemoryId, MemoryRecord, MemoryRevision, PayloadState,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transfer {
    format: Format,
    owner: String,
    history_generation: String,
    revisions: Vec<Revision>,
}

#[derive(Deserialize)]
enum Format {
    #[serde(rename = "reliquary-d01-synthetic-phy-memory-transfer-v1")]
    Memory,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    memory_id: String,
    body_id: String,
    title: String,
    content: String,
    memory_type: String,
    authority_kind: String,
    lifecycle_state: String,
    created_at_ns: String,
    source_time_ns: String,
    revision: String,
    memory_version: String,
    global_version: String,
    transaction_time_ns: String,
    updated_at_ns: String,
    mutation_id: String,
    category: String,
    temporal_status: String,
    archived: bool,
}

pub struct Publication {
    pub global_version: u64,
    pub transaction_time_ns: i64,
    pub revision: MemoryRevision,
}

impl Transfer {
    pub fn owner(&self) -> Result<[u8; 16], &'static str> {
        decode_hex(&self.owner)
    }

    pub fn generation(&self) -> Result<[u8; 32], &'static str> {
        decode_hex(&self.history_generation)
    }

    pub fn publications(self) -> Result<Vec<Publication>, &'static str> {
        let Format::Memory = self.format;
        if !(1..=64).contains(&self.revisions.len()) {
            return Err("synthetic Memory revision count is outside bounds");
        }
        self.revisions
            .into_iter()
            .map(Publication::try_from)
            .collect()
    }
}

impl TryFrom<Revision> for Publication {
    type Error = &'static str;

    fn try_from(input: Revision) -> Result<Self, Self::Error> {
        Ok(Self {
            global_version: decimal(&input.global_version)?,
            transaction_time_ns: decimal(&input.transaction_time_ns)?,
            revision: MemoryRevision {
                record: MemoryRecord {
                    id: MemoryId(decode_hex(&input.memory_id)?),
                    body_id: MemoryBodyId(decode_hex(&input.body_id)?),
                    revision: decimal(&input.revision)?,
                    memory_version: decimal(&input.memory_version)?,
                    category: input.category,
                    memory_type: input.memory_type,
                    authority_kind: input.authority_kind,
                    temporal_status: input.temporal_status,
                    scope: "private".into(),
                    lifecycle_state: input.lifecycle_state,
                    archived: input.archived,
                    parent_id: None,
                    superseded_by: None,
                    source_node_id: None,
                    content_source_conversation_id: None,
                    content_source_node_id: None,
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                    source_episode_id: None,
                    source_ref: None,
                    source_time_ns: Some(decimal(&input.source_time_ns)?),
                    temporal_inference: None,
                    mutation_id: input.mutation_id,
                    created_at_ns: decimal(&input.created_at_ns)?,
                    updated_at_ns: decimal(&input.updated_at_ns)?,
                },
                payload: PayloadState::Available(MemoryBody {
                    title: input.title,
                    content: input.content,
                }),
            },
        })
    }
}
