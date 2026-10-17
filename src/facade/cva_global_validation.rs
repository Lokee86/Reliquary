use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::vector_generation_store::VectorGenerationStore;
use crate::{Archive, CvaError};

pub(crate) fn validate_semantic_global_versions(
    archive: &Archive,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &GraphStore,
    vector_generations: &VectorGenerationStore,
) -> Result<(), CvaError> {
    let mut versions: Vec<u64> = archive
        .record_versions()
        .iter()
        .map(|record| record.global_version)
        .chain(
            memories
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(
            entities
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(
            relationships
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(memories.tombstone_global_versions())
        .chain(entities.tombstone_global_versions())
        .chain(relationships.tombstone_global_versions())
        .chain(graph.transaction_global_versions().iter().copied())
        .chain(
            vector_generations
                .generations()
                .iter()
                .map(|generation| generation.global_version),
        )
        .collect();
    versions.sort_unstable();
    for pair in versions.windows(2) {
        if pair[0] == pair[1] {
            return Err(CvaError::SemanticGlobalVersionConflict(pair[0]));
        }
    }
    Ok(())
}
