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
            archive: &self.owner.archive,
            memories: &self.owner.memories,
            settlement: crate::history::SettlementCompletionEvidence::new(
                &self.owner.archive,
                &self.owner.memories,
                &self.owner.graph,
                &self.owner.freshness,
                &self.owner.dream_freshness,
            ),
            insomnia: crate::insomnia::import_readiness::ImportReadiness::new(&self.owner.insomnia),
            dream: crate::dream_readiness::DreamReadiness::new(
                &self.owner.dream_freshness,
                &self.owner.processing_epochs,
            ),
            perception: crate::perception_import_readiness::ImportReadiness::new(
                &self.owner.memories,
                &self.owner.entity_resolutions,
            ),
            watermarks: self.import_watermarks(),
            turn: self.owner.archive.rel_turn_count(),
        };
        let mut access =
            crate::owner_lifecycle::ImportCompletionAccess::new(&mut self.owner.container);
        crate::imports::owner_completion::complete_with_views(
            &mut access,
            &mut self.owner.import_epochs,
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
