//! Owner-local typed Graph and derived Community state over a Container stream.
//!
//! Callers supply endpoint facts admitted under their owner context. This crate
//! owns graph shape, ordering and publication; foreign record truth and semantic
//! producer policy remain with product lifecycle and the corresponding owners.

/// Canonical foreign semantic identity; ownership and existence are validated by callers.
pub type MemoryId = [u8; 32];
pub type EntityId = [u8; 32];

/// Primitive endpoint facts supplied by the product's owner admission seam.
///
/// These values do not discover or authorize foreign records. The caller must
/// resolve them from its current/historical owning stores under the same held
/// owner/Container context as the subsequent operation. Constructing this value
/// is an explicit assertion of those facts, never permission to read another owner.
#[derive(Clone, Debug, Default)]
pub struct GraphAdmission {
    current_memories: std::collections::HashSet<MemoryId>,
    historical_memories: std::collections::HashSet<MemoryId>,
    current_entities: std::collections::HashSet<EntityId>,
    historical_entities: std::collections::HashSet<EntityId>,
    principal_entities: std::collections::HashSet<EntityId>,
}
impl GraphAdmission {
    pub fn for_changes(
        current_memories: impl IntoIterator<Item = MemoryId>,
        current_entities: impl IntoIterator<Item = (EntityId, bool)>,
    ) -> Self {
        let current_memories = current_memories.into_iter().collect();
        let mut entities = std::collections::HashSet::new();
        let mut principals = std::collections::HashSet::new();
        for (id, principal) in current_entities {
            entities.insert(id);
            if principal {
                principals.insert(id);
            }
        }
        Self {
            current_memories,
            current_entities: entities,
            principal_entities: principals,
            ..Self::default()
        }
    }
    pub fn for_replay(
        current_memories: impl IntoIterator<Item = MemoryId>,
        historical_memories: impl IntoIterator<Item = MemoryId>,
        current_entities: impl IntoIterator<Item = (EntityId, bool)>,
        historical_entities: impl IntoIterator<Item = EntityId>,
        historical_principals: impl IntoIterator<Item = EntityId>,
    ) -> Self {
        let mut facts = Self::for_changes(current_memories, current_entities);
        facts.historical_memories = historical_memories.into_iter().collect();
        facts.historical_entities = historical_entities.into_iter().collect();
        facts.principal_entities.extend(historical_principals);
        facts
    }
}

mod semantic_node_model;
pub use semantic_node_model::{SemanticNodeKind, SemanticNodeRef};
mod graph_model;
pub use graph_model::*;
mod graph_error;
pub use graph_error::GraphError;
pub mod graph_codec;
#[cfg(test)]
mod graph_codec_tests;
pub mod graph_compact_codec;
pub mod graph_compact_contract;
pub mod graph_rebuild;
pub use graph_rebuild::GraphOpenState;
pub mod graph_store;
pub use graph_store::{GraphStore, validate_relation_contract};
mod community_model;
mod graph_query;
pub use community_model::*;
mod community_error;
pub use community_error::CommunityError;
pub mod community_codec;
mod community_leiden;
pub mod community_lineage;
pub mod community_scan_merge;
mod community_scan_merge_plan;
mod community_scan_merge_reduce;
mod community_scan_merge_workers;
pub mod community_store;
pub use community_store::{CommunityOpenState, CommunityStore};
#[cfg(test)]
mod community_lineage_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
mod community_scan_merge_bench;
#[cfg(test)]
mod community_scan_merge_tests;
#[cfg(test)]
mod graph_compact_codec_tests;
