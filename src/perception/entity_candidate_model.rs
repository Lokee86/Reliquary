use crate::{
    Entity, GraphError, MemoryEntityMention, MemoryEntityMentionKey, MemoryError, MemoryId,
};
use std::fmt;

pub const DEFAULT_ENTITY_CANDIDATE_LEXICAL_MEMORIES: usize = 24;
pub const DEFAULT_ENTITY_CANDIDATE_GRAPH_NEIGHBORS: usize = 24;
pub const DEFAULT_ENTITY_CANDIDATE_SUPPORT_MEMORIES: usize = 4;
pub const MAX_ENTITY_ADMISSION_SURFACE_MEMORIES: usize = 32;
pub const MAX_ENTITY_CANDIDATE_CONTEXT_TERMS: usize = 16;
pub const MAX_ENTITY_CANDIDATE_SURFACE_MATCHES: usize = 64;
pub const MAX_ENTITY_CANDIDATE_LEXICAL_MEMORIES: usize = 64;
pub const MAX_ENTITY_CANDIDATE_GRAPH_NEIGHBORS: usize = 64;
pub const MAX_ENTITY_CANDIDATE_SUPPORT_MEMORIES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityCandidateConfig {
    pub max_candidates: usize,
    pub lexical_memory_limit: usize,
    pub graph_neighbor_limit: usize,
    pub support_memory_limit: usize,
}

impl Default for EntityCandidateConfig {
    fn default() -> Self {
        Self {
            max_candidates: crate::MAX_ENTITY_RESOLUTION_CANDIDATES,
            lexical_memory_limit: DEFAULT_ENTITY_CANDIDATE_LEXICAL_MEMORIES,
            graph_neighbor_limit: DEFAULT_ENTITY_CANDIDATE_GRAPH_NEIGHBORS,
            support_memory_limit: DEFAULT_ENTITY_CANDIDATE_SUPPORT_MEMORIES,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityCandidate {
    pub entity: Entity,
    pub exact_surface: bool,
    pub normalized_surface: bool,
    pub alias_surface: bool,
    pub source_association: bool,
    pub lexical_memory_hits: usize,
    pub graph_neighbor_hits: usize,
    pub best_lexical_score: f64,
    pub supporting_memory_ids: Vec<MemoryId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityCandidateSet {
    pub key: MemoryEntityMentionKey,
    pub mention: MemoryEntityMention,
    pub candidates: Vec<EntityCandidate>,
    pub admission_context_memory_ids: Vec<MemoryId>,
    pub lexical_memories_examined: usize,
    pub graph_neighbors_examined: usize,
}

#[derive(Debug)]
pub enum EntityCandidateError {
    InvalidConfig,
    MissingMention(MemoryEntityMentionKey),
    Memory(MemoryError),
    Graph(GraphError),
    Entity(crate::EntityError),
}

impl fmt::Display for EntityCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig => f.write_str("invalid Entity candidate retrieval config"),
            Self::MissingMention(key) => write!(
                f,
                "missing Entity mention for memory {:02x?} {:?} {}..{}",
                key.memory_id.0, key.field, key.start_byte, key.end_byte
            ),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::Entity(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EntityCandidateError {}

impl From<MemoryError> for EntityCandidateError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<GraphError> for EntityCandidateError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

impl From<crate::EntityError> for EntityCandidateError {
    fn from(value: crate::EntityError) -> Self {
        Self::Entity(value)
    }
}

impl From<reliquary_perception_store::EntityError> for EntityCandidateError {
    fn from(value: reliquary_perception_store::EntityError) -> Self {
        Self::Entity(value.into())
    }
}
