//! Thin Cva adapter for native Import completion.
use crate::imports::{ImportCompletionOutcome, ImportEpochStore};
use crate::{Cva, CvaError};

impl Cva {
    /// Exclusive offline completion. Import owns recovery, refusal and
    /// publication ordering; this adapter supplies exact owner views.
    pub fn complete_native_import(
        &mut self,
        import_id: &str,
        now_ns: i64,
    ) -> Result<ImportCompletionOutcome, CvaError> {
        let views = crate::imports::owner_completion::CompletionReadViews {
            archive: &self.archive,
            memories: &self.memories,
            graph: &self.graph,
            freshness: &self.freshness,
            dream_freshness: &self.dream_freshness,
            insomnia: &self.insomnia,
            processing_epochs: &self.processing_epochs,
            entity_resolutions: &self.entity_resolutions,
            watermarks: self.import_watermarks(),
            turn: self.archive.rel_turn_count(),
        };
        let mut access = crate::owner_lifecycle::ImportCompletionAccess::new(&mut self.container);
        crate::imports::owner_completion::complete_with_views(
            &mut access,
            &mut self.import_epochs,
            import_id,
            now_ns,
            views,
        )
    }

    /// Rebuild exact accepted epoch state from the bounded REL history source.
    pub(crate) fn completion_epochs_from_bytes(&self) -> Result<ImportEpochStore, CvaError> {
        let access = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        crate::imports::completion_coordinator::recover_epochs(&access, &self.archive)
    }
}
