//! PHY reclamation authorization and post-copy semantic invariants.
use super::reclamation::{StorageReclamationReport, reclaim_container};
use crate::container::Container;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::OwnerPhylacteryError;
use crate::relationship_store::RelationshipStore;
use std::{fs, path::Path};

pub(crate) struct PhylacteryReclamationParts<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) relationships: &'a RelationshipStore,
    pub(crate) graph: &'a GraphStore,
}

pub(crate) fn reclaim(
    parts: PhylacteryReclamationParts<'_>,
    output_path: &Path,
) -> Result<StorageReclamationReport, OwnerPhylacteryError> {
    let expected = clocks(
        parts.container,
        parts.memories,
        parts.entities,
        parts.relationships,
        parts.graph,
    );
    let (report, output) =
        reclaim_container(parts.container, output_path).map_err(OwnerPhylacteryError::Repack)?;
    let output_guard = output.read_source()?;
    let reopened = super::phylactery_copy::reopen(output).map_err(|error| {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        error
    })?;
    let actual = clocks(
        &reopened.container,
        &reopened.memories,
        &reopened.entities,
        &reopened.relationships,
        &reopened.graph,
    );
    drop(reopened);
    if actual != expected {
        if output_guard.validate_path_identity().is_ok() {
            let _ = fs::remove_file(output_path);
        }
        return Err(OwnerPhylacteryError::Repack(
            "reclaimed PHY changed owner or semantic version state".into(),
        ));
    }
    output_guard.validate_path_identity()?;
    Ok(report)
}

fn clocks(
    container: &Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &GraphStore,
) -> (Option<[u8; 16]>, u64, u64, u64, u64, u64) {
    (
        container.owner_uuid(),
        container.latest_version(),
        memories.memory_version(),
        entities.entity_version(),
        relationships.relationship_version(),
        graph.graph_version(),
    )
}
