use crate::maintenance::reclamation::{
    ReliquaryReclamationParts, StorageReclamationReport, reclaim_reliquary_parts,
};
use crate::{Cva, CvaError, Phylactery, PhylacteryError};
use std::path::Path;

impl Cva {
    pub fn reclaim_storage(
        &mut self,
        output_path: impl AsRef<Path>,
    ) -> Result<StorageReclamationReport, CvaError> {
        reclaim_reliquary_parts(
            ReliquaryReclamationParts {
                container: &mut self.owner.container,
                archive: &self.owner.archive,
                memories: &self.owner.memories,
                entities: &self.owner.entities,
                relationships: &self.owner.relationships,
                graph: &self.owner.graph,
                vector_generations: &self.owner.vector_generations,
                freshness: &self.owner.freshness,
                import_epochs: &self.owner.import_epochs,
            },
            output_path.as_ref(),
        )
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
                    &mut self.owner.container,
                )
                .map_err(PhylacteryError::Repack)?,
                memories: &self.owner.memories,
                entities: &self.owner.entities,
                relationships: &self.owner.relationships,
                graph: &self.owner.graph,
            },
            output_path.as_ref(),
        )
    }
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
