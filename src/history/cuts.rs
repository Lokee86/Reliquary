//! Compose independent owner-local versions at a global ordering point.

use crate::{
    archive::Archive, entity_store::EntityStore, graph_store::GraphStore,
    memory_store::MemoryStore, relationship_store::RelationshipStore,
    vector_generation_store::VectorGenerationStore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalOwnerCuts {
    pub(crate) archive: u64,
    pub(crate) memory: u64,
    pub(crate) entity: u64,
    pub(crate) relationship: u64,
    pub(crate) graph: u64,
    pub(crate) memory_graph: u64,
    pub(crate) vector_generation: u64,
}

pub(crate) struct OwnerCutFacts<'a> {
    pub(crate) archive: Option<&'a Archive>,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) relationships: &'a RelationshipStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) vector_generations: Option<&'a VectorGenerationStore>,
    pub(crate) memory_graph_positions: &'a [u64],
}

pub(crate) fn owner_cuts(
    source_head: u64,
    global: u64,
    facts: OwnerCutFacts<'_>,
) -> Result<GlobalOwnerCuts, String> {
    if global > source_head {
        return Err("global cut exceeds retained owner head".into());
    }

    let archive = facts
        .archive
        .map(|archive| {
            archive
                .record_versions()
                .iter()
                .take_while(|version| version.global_version <= global)
                .map(|version| version.archive_version)
                .last()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let memory = facts.memories.local_version_at_global_cut(global);
    let entity = facts.entities.local_version_at_global_cut(global);
    let relationship = facts.relationships.local_version_at_global_cut(global);

    let graph = facts
        .graph
        .transaction_global_versions()
        .iter()
        .take_while(|version| **version <= global)
        .count() as u64;
    let memory_graph = facts
        .memory_graph_positions
        .iter()
        .copied()
        .filter(|position| *position <= graph)
        .last()
        .unwrap_or(0);

    let vector_generation = facts
        .vector_generations
        .map(|store| {
            store
                .generations()
                .iter()
                .filter(|generation| generation.global_version <= global)
                .map(|generation| generation.vector_version)
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);

    Ok(GlobalOwnerCuts {
        archive,
        memory,
        entity,
        relationship,
        graph,
        memory_graph,
        vector_generation,
    })
}
