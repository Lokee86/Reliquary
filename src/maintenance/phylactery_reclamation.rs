//! PHY reclamation authorization and post-copy semantic invariants.
use super::reclamation::{StorageReclamationReport, reclaim_phylactery};
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::OwnerPhylacteryError;
use crate::owner_lifecycle::PhylacteryReclamationSource;
use crate::relationship_store::RelationshipStore;
use std::{fs, path::Path};

pub(crate) struct PhylacteryReclamationParts<'a> {
    pub(crate) source: PhylacteryReclamationSource<'a>,
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
        parts.source.owner_uuid(),
        parts.source.latest_version(),
        parts.memories,
        parts.entities,
        parts.relationships,
        parts.graph,
    );
    let (report, output) =
        reclaim_phylactery(parts.source, output_path).map_err(OwnerPhylacteryError::Repack)?;
    let output_guard = output.read_source()?;
    let reopened = output
        .open_owner()
        .and_then(super::phylactery_copy::validate_owner)
        .map_err(|error| {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            error
        })?;
    let actual = clocks(
        reopened.owner_uuid(),
        reopened.latest_global_version(),
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
    owner_uuid: Option<[u8; 16]>,
    latest_global: u64,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &GraphStore,
) -> (Option<[u8; 16]>, u64, u64, u64, u64, u64) {
    (
        owner_uuid,
        latest_global,
        memories.memory_version(),
        entities.entity_version(),
        relationships.relationship_version(),
        graph.graph_version(),
    )
}
