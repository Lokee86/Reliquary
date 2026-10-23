use crate::imports::*;
use crate::{Branch, Cva, EpisodeConfig, EpisodeOrigin, InsomniaPriority};

impl Cva {
    /// Offline/exclusive native import. Mounted management must compose this through
    /// its exact REL mutation gate; this method never opens a second runtime.
    /// Retry the same normalized input after partial failure. A receipt acknowledges
    /// all source records, branch heads and Import-priority queue entries after sync.
    pub fn import_native(
        &mut self,
        input: &NativeImport,
        now_ns: i64,
    ) -> Result<ImportReceipt, ImportError> {
        let watermarks = self.import_watermarks();
        let mut activity = Vec::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::imports::native_commit::import_native(
                crate::imports::native_commit::NativeImportParts::new(
                    &mut self.container,
                    &mut self.archive,
                    &self.project_files,
                    &mut self.insomnia,
                    &mut self.import_epochs,
                ),
                input,
                now_ns,
                watermarks,
                &mut activity,
            )
        }));
        for turn in activity {
            self.freshness_on_accepted_activity(turn);
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_import_write_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.container.set_owner_write_scope(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        self.container.set_owner_write_scope(false);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub fn native_import_receipts(&self) -> Vec<ImportReceipt> {
        self.archive.import_receipts.values().cloned().collect()
    }

    pub fn open_import_epoch(&self) -> Option<ImportStart> {
        self.import_epochs.open().cloned()
    }

    pub fn import_epoch_status(&self, import_id: &str) -> Option<ImportEpochStatus> {
        self.import_epochs.status(import_id)
    }

    pub fn import_epoch_statuses(&self) -> Vec<ImportEpochStatus> {
        self.import_epochs.statuses()
    }

    pub(crate) fn import_watermarks(&self) -> ImportWatermarks {
        ImportWatermarks {
            global_version: self.latest_global_version(),
            archive_version: self.archive_version(),
            memory_version: self.memory_version(),
            entity_version: self.entities.entity_version(),
            relationship_version: self.relationships.relationship_version(),
            graph_version: self.graph.graph_version(),
            memory_graph_version: self.graph.memory_graph_version(),
            vector_version: self.vector_generations.vector_version(),
        }
    }
}
