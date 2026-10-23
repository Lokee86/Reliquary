//! Public adapter for explicit REL history pruning.
use crate::{Cva, CvaError, HistoryPruneReport, HistoryPruneRequest};
use std::path::Path;

impl Cva {
    /// Write a separately validated copy with explicitly selected terminal conversation bodies removed.
    pub fn prune_tombstoned_history(
        &mut self,
        output_path: impl AsRef<Path>,
        request: HistoryPruneRequest,
    ) -> Result<HistoryPruneReport, CvaError> {
        crate::maintenance::prune_reliquary(
            crate::maintenance::ReliquaryPruneParts {
                container: &mut self.container,
                archive: &self.archive,
                memories: &self.memories,
                graph: &self.graph,
                entities: &self.entities,
                relationships: &self.relationships,
                packed_vectors: &self.packed_vectors,
                memory_vectors: &self.memory_vectors,
                archive_vectors: &self.archive_vectors,
                compatibility_profiles: &self.compatibility_profiles,
                vector_generations: &self.vector_generations,
                owner_maintenance: &self.owner_maintenance,
                freshness: &self.freshness,
                dream_freshness: &self.dream_freshness,
                import_epochs: &self.import_epochs,
            },
            output_path.as_ref(),
            request,
        )
    }
}

#[cfg(test)]
#[path = "history_prune_tests.rs"]
mod tests;
