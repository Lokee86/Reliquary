use crate::maintenance::reclamation::{StorageReclamationReport, reclaim_reliquary};
use crate::{Cva, CvaError, Phylactery, PhylacteryError};
use std::fs;
use std::path::Path;

impl Cva {
    pub fn reclaim_storage(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<StorageReclamationReport, CvaError> {
        if self
            .freshness
            .import_settlements()
            .iter()
            .any(|s| s.semantic.is_none())
        {
            return Err(CvaError::Repack(
                "current import settlement semantic evidence is missing".into(),
            ));
        }
        let expected = SemanticState {
            owner_uuid: self.owner_uuid(),
            global_version: self.latest_global_version(),
            memory_version: self.memory_version(),
            entity_version: self.entity_version(),
            relationship_version: self.relationship_version(),
            graph_version: self.graph_version(),
            archive_version: Some(self.archive_version()),
            vector_version: Some(self.vector_version()),
        };
        let (report, output) = reclaim_reliquary(
            crate::owner_lifecycle::ReliquaryReclamationSource::prepare(&mut self.container)
                .map_err(CvaError::Repack)?,
            output_path.as_ref(),
        )
        .map_err(CvaError::Repack)?;
        let output = output.into_reliquary_container();
        let output_guard = output.read_source()?;
        let reopened = Cva::open_from_container(output).map_err(|error| {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path.as_ref());
            }
            error
        })?;
        if let Err(error) = self.validate_historical_import_copy(&reopened) {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path.as_ref());
            }
            return Err(error);
        }
        let actual = SemanticState {
            owner_uuid: reopened.owner_uuid(),
            global_version: reopened.latest_global_version(),
            memory_version: reopened.memory_version(),
            entity_version: reopened.entity_version(),
            relationship_version: reopened.relationship_version(),
            graph_version: reopened.graph_version(),
            archive_version: Some(reopened.archive_version()),
            vector_version: Some(reopened.vector_version()),
        };
        let activity_matches = self.archive.activity_history_matches(&reopened.archive);
        drop(reopened);
        if actual != expected || !activity_matches {
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path.as_ref());
            }
            return Err(CvaError::Repack(
                "reclaimed REL changed owner, semantic versions or activity history".into(),
            ));
        }
        output_guard.validate_path_identity()?;
        Ok(report)
    }
}

impl Phylactery {
    pub fn reclaim_storage(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<StorageReclamationReport, PhylacteryError> {
        crate::maintenance::phylactery_reclamation::reclaim(
            crate::maintenance::phylactery_reclamation::PhylacteryReclamationParts {
                source: crate::owner_lifecycle::PhylacteryReclamationSource::prepare(
                    &mut self.container,
                )
                .map_err(PhylacteryError::Repack)?,
                memories: &self.memories,
                entities: &self.entities,
                relationships: &self.relationships,
                graph: &self.graph,
            },
            output_path.as_ref(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticState {
    owner_uuid: Option<[u8; 16]>,
    global_version: u64,
    memory_version: u64,
    entity_version: u64,
    relationship_version: u64,
    graph_version: u64,
    archive_version: Option<u64>,
    vector_version: Option<u64>,
}

#[cfg(test)]
use crate::owner_lifecycle::relocation::relocate_payload;
#[cfg(test)]
#[path = "storage_reclamation_entity_compact_tests.rs"]
mod entity_compact_tests;

#[cfg(test)]
#[path = "storage_reclamation_relationship_compact_tests.rs"]
mod relationship_compact_tests;

#[cfg(test)]
#[path = "storage_reclamation_memory_publication_tests.rs"]
mod memory_publication_tests;

#[cfg(test)]
#[path = "storage_reclamation_entity_publication_tests.rs"]
mod entity_publication_tests;

#[cfg(test)]
#[path = "storage_reclamation_relationship_publication_tests.rs"]
mod relationship_publication_tests;

#[cfg(test)]
#[path = "graph_compact_maintenance_tests.rs"]
mod compact_graph_tests;

#[cfg(test)]
#[path = "storage_reclamation_graph_publication_tests.rs"]
mod graph_publication_tests;
