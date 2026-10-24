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
                container: &mut self.owner.container,
                archive: &self.owner.archive,
                memories: &self.owner.memories,
                graph: &self.owner.graph,
                entities: &self.owner.entities,
                relationships: &self.owner.relationships,
                packed_vectors: &self.owner.packed_vectors,
                memory_vectors: &self.owner.memory_vectors,
                archive_vectors: &self.owner.archive_vectors,
                compatibility_profiles: &self.owner.compatibility_profiles,
                vector_generations: &self.owner.vector_generations,
                owner_maintenance: &self.owner.owner_maintenance,
                freshness: &self.owner.freshness,
                dream_freshness: &self.owner.dream_freshness,
                import_epochs: &self.owner.import_epochs,
            },
            output_path.as_ref(),
            request,
        )
    }
}

#[cfg(test)]
#[path = "history_prune_tests.rs"]
mod tests;
