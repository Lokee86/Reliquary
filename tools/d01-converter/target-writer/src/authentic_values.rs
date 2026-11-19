use crate::transfer::decode_hex;
use reliquary_graph::{NodeRef, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{
    MemoryBody, MemoryBodyId, MemoryId, MemoryRecord, MemoryRoutingField, MemoryRoutingMention,
    MemoryRoutingMetadata, MemorySourceRef,
};
use serde::Deserialize;

type Invalid = &'static str;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    body_id: String,
    title: String,
    content: String,
}

impl Body {
    pub fn into_native(self) -> Result<(MemoryBodyId, MemoryBody), Invalid> {
        Ok((
            MemoryBodyId(decode_hex(&self.body_id)?),
            MemoryBody {
                title: self.title,
                content: self.content,
            },
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    id: String,
    revision: u64,
    body_id: String,
    memory_version: u64,
    pub global_version: u64,
    pub transaction_time_ns: Option<i64>,
    category: String,
    memory_type: String,
    authority_kind: String,
    temporal_status: String,
    scope: String,
    lifecycle_state: String,
    archived: bool,
    parent_id: Option<String>,
    superseded_by: Option<String>,
    source_node_id: Option<String>,
    content_source_conversation_id: Option<String>,
    content_source_node_id: Option<String>,
    grounding_source_conversation_id: Option<String>,
    grounding_source_node_id: Option<String>,
    source_episode_id: Option<String>,
    source_ref: Option<Source>,
    source_time_ns: Option<i64>,
    temporal_inference: Option<serde_json::Value>,
    mutation_id: String,
    created_at_ns: i64,
    updated_at_ns: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    owner_id: String,
    principal_id: Option<String>,
    source_episode_id: String,
    source_node_id: String,
    content_source_conversation_id: Option<String>,
    content_source_node_id: Option<String>,
    grounding_source_conversation_id: Option<String>,
    grounding_source_node_id: Option<String>,
}

fn optional_id(input: Option<String>) -> Result<Option<[u8; 32]>, Invalid> {
    input.as_deref().map(decode_hex).transpose()
}

impl Memory {
    pub fn into_native(self) -> Result<MemoryRecord, Invalid> {
        if self.temporal_inference.is_some() {
            return Err("unsupported retained Memory temporal inference");
        }
        let source_ref = self
            .source_ref
            .map(|source| {
                Ok(MemorySourceRef {
                    owner_id: source.owner_id,
                    principal_id: source.principal_id,
                    source_episode_id: decode_hex(&source.source_episode_id)?,
                    source_node_id: source.source_node_id,
                    content_source_conversation_id: source.content_source_conversation_id,
                    content_source_node_id: source.content_source_node_id,
                    grounding_source_conversation_id: source.grounding_source_conversation_id,
                    grounding_source_node_id: source.grounding_source_node_id,
                })
            })
            .transpose()?;
        Ok(MemoryRecord {
            id: MemoryId(decode_hex(&self.id)?),
            revision: self.revision,
            body_id: MemoryBodyId(decode_hex(&self.body_id)?),
            memory_version: self.memory_version,
            category: self.category,
            memory_type: self.memory_type,
            authority_kind: self.authority_kind,
            temporal_status: self.temporal_status,
            scope: self.scope,
            lifecycle_state: self.lifecycle_state,
            archived: self.archived,
            parent_id: optional_id(self.parent_id)?.map(MemoryId),
            superseded_by: optional_id(self.superseded_by)?.map(MemoryId),
            source_node_id: self.source_node_id,
            content_source_conversation_id: self.content_source_conversation_id,
            content_source_node_id: self.content_source_node_id,
            grounding_source_conversation_id: self.grounding_source_conversation_id,
            grounding_source_node_id: self.grounding_source_node_id,
            source_episode_id: optional_id(self.source_episode_id)?,
            source_ref,
            source_time_ns: self.source_time_ns,
            temporal_inference: None,
            mutation_id: self.mutation_id,
            created_at_ns: self.created_at_ns,
            updated_at_ns: self.updated_at_ns,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routing {
    memory_id: String,
    body_id: String,
    mentions: Vec<Mention>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mention {
    field: u8,
    start: u32,
    end: u32,
    text: String,
}

impl Routing {
    pub fn into_native(self) -> Result<MemoryRoutingMetadata, Invalid> {
        let mentions = self
            .mentions
            .into_iter()
            .map(|item| {
                Ok(MemoryRoutingMention {
                    field: match item.field {
                        1 => MemoryRoutingField::Title,
                        2 => MemoryRoutingField::Content,
                        _ => return Err("unknown Memory routing field"),
                    },
                    start_byte: item.start,
                    end_byte: item.end,
                    text: item.text,
                })
            })
            .collect::<Result<Vec<_>, Invalid>>()?;
        Ok(MemoryRoutingMetadata {
            memory_id: MemoryId(decode_hex(&self.memory_id)?),
            body_id: MemoryBodyId(decode_hex(&self.body_id)?),
            mentions,
        })
    }
}

#[derive(Deserialize)]
pub struct GraphChange((u8, String), (u8, String), u16, bool, u8);

impl GraphChange {
    pub fn into_native(self) -> Result<RelationChange, Invalid> {
        fn node((kind, id): (u8, String)) -> Result<NodeRef, Invalid> {
            match kind {
                1 => Ok(NodeRef::Memory(decode_hex(&id)?)),
                2 => Ok(NodeRef::Entity(decode_hex(&id)?)),
                _ => Err("unknown Graph endpoint kind"),
            }
        }
        Ok(RelationChange {
            source: node(self.0)?,
            target: node(self.1)?,
            kind: match self.2 {
                1 => RelationKind::Topical,
                2 => RelationKind::Factual,
                3 => RelationKind::Causal,
                4 => RelationKind::Recurrent,
                5 => RelationKind::References,
                6 => RelationKind::DuplicateOf,
                7 => RelationKind::Supersedes,
                8 => RelationKind::StructuralParent,
                100 => RelationKind::EntityAssociation,
                101 => RelationKind::PrincipalAssociation,
                _ => return Err("unknown Graph relation kind"),
            },
            active: self.3,
            origin: match self.4 {
                1 => RelationOrigin::Dream,
                2 => RelationOrigin::User,
                3 => RelationOrigin::Perception,
                _ => return Err("unknown Graph relation origin"),
            },
        })
    }
}
