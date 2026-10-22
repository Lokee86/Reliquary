//! Outward API adapters for canonical History proofs and Maintenance publication.
use crate::{Phylactery, PhylacteryError};
use std::path::Path;

impl Phylactery {
    /// Write a separately validated copy with explicitly selected terminal PHY payloads removed.
    pub fn prune_tombstoned_history(
        &mut self,
        output_path: impl AsRef<Path>,
        request: crate::HistoryPruneRequest,
    ) -> Result<crate::HistoryPruneReport, PhylacteryError> {
        crate::maintenance::prune_phylactery(
            crate::maintenance::PhylacteryPruneParts {
                owner_maintenance: &self.owner_maintenance,
                container: &mut self.container,
                memories: &self.memories,
                graph: &self.graph,
                packed_vectors: &self.packed_vectors,
                memory_vectors: &self.memory_vectors,
                compatibility_profiles: &self.compatibility_profiles,
                entities: &self.entities,
                relationships: &self.relationships,
            },
            output_path.as_ref(),
            request.targets,
        )
    }
    pub(crate) fn validate_prune_evidence(&mut self) -> Result<(), PhylacteryError> {
        crate::history::validate_phylactery_prune_evidence(
            &self.owner_maintenance,
            &crate::owner_lifecycle::HistorySource::new(&self.container)
                .map_err(|error| PhylacteryError::Repack(error.to_string()))?,
            &self.graph,
            &self.memories,
            &self.entities,
            &self.relationships,
            &self.compatibility_profiles,
            &self.memory_vectors,
            &self.packed_vectors,
        )
    }
}
