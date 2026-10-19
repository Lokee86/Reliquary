//! Test-only mapping from a CVA global ordering point to independent local cuts.
//! Global versions order publications; they do not introduce owner ancestry.

use crate::{Cva, Phylactery};

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

pub(crate) fn reliquary_global_cuts(cva: &Cva, global: u64) -> Result<GlobalOwnerCuts, String> {
    if global > cva.latest_global_version() {
        return Err("global cut exceeds retained owner head".into());
    }

    let archive = cva
        .archive
        .record_versions()
        .iter()
        .take_while(|version| version.global_version <= global)
        .map(|version| version.archive_version)
        .last()
        .unwrap_or(0);

    let memory = cva
        .memories
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.memory_version)
        .chain(cva.memories.tombstoned_ids().into_iter().filter_map(|id| {
            match cva.memories.lifecycle_state(id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let entity = cva
        .entities
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.entity_version)
        .chain(cva.entities.records().iter().filter_map(|record| {
            match cva.entities.lifecycle_state(record.id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let relationship = cva
        .relationships
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.relationship_version)
        .chain(cva.relationships.records().iter().filter_map(|record| {
            match cva.relationships.lifecycle_state(record.id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&cva.container, &cva.graph)?;
    let graph = cva
        .graph
        .transaction_global_versions()
        .iter()
        .take_while(|version| **version <= global)
        .count() as u64;
    let memory_graph = graph_history
        .memory_graph_positions()
        .into_iter()
        .filter(|position| *position <= graph)
        .last()
        .unwrap_or(0);

    let vector_generation = cva
        .vector_generations
        .generations()
        .iter()
        .filter(|generation| generation.global_version <= global)
        .map(|generation| generation.vector_version)
        .max()
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

pub(crate) fn phylactery_global_cuts(
    phy: &Phylactery,
    global: u64,
) -> Result<GlobalOwnerCuts, String> {
    if global > phy.latest_global_version() {
        return Err("global cut exceeds retained owner head".into());
    }

    let memory = phy
        .memories
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.memory_version)
        .chain(phy.memories.tombstoned_ids().into_iter().filter_map(|id| {
            match phy.memories.lifecycle_state(id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let entity = phy
        .entities
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.entity_version)
        .chain(phy.entities.records().iter().filter_map(|record| {
            match phy.entities.lifecycle_state(record.id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let relationship = phy
        .relationships
        .records()
        .iter()
        .filter(|record| record.global_version <= global)
        .map(|record| record.relationship_version)
        .chain(phy.relationships.records().iter().filter_map(|record| {
            match phy.relationships.lifecycle_state(record.id).ok()? {
                crate::owner_lifecycle::LifecycleState::Tombstoned(coordinates)
                    if coordinates.global_version? <= global =>
                {
                    coordinates.domain_version
                }
                _ => None,
            }
        }))
        .max()
        .unwrap_or(0);

    let graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&phy.container, &phy.graph)?;
    let graph = phy
        .graph
        .transaction_global_versions()
        .iter()
        .take_while(|version| **version <= global)
        .count() as u64;
    let memory_graph = graph_history
        .memory_graph_positions()
        .into_iter()
        .filter(|position| *position <= graph)
        .last()
        .unwrap_or(0);

    Ok(GlobalOwnerCuts {
        archive: 0,
        memory,
        entity,
        relationship,
        graph,
        memory_graph,
        vector_generation: 0,
    })
}
