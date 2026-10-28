use reliquary_memory_core::{MemoryEntityMention, MemoryEntityMentionKey, MemoryId};
use reliquary_perception_store::Entity;

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
            max_candidates: reliquary_perception_store::MAX_ENTITY_RESOLUTION_CANDIDATES,
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
