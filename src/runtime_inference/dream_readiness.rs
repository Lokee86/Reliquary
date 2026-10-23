use crate::MemoryId;
use crate::freshness_dream_journal::DreamFreshnessJournal;

/// Narrow Dream-owned read view for Import's first-cycle readiness checks.
pub(crate) struct DreamReadiness<'a> {
    journal: &'a DreamFreshnessJournal,
    processing: &'a crate::processing_epoch_store::ProcessingEpochStore,
}

impl<'a> DreamReadiness<'a> {
    pub(crate) fn new(
        journal: &'a DreamFreshnessJournal,
        processing: &'a crate::processing_epoch_store::ProcessingEpochStore,
    ) -> Self {
        Self {
            journal,
            processing,
        }
    }

    pub(crate) fn initial_pass_id(&self, owner: [u8; 16], id: MemoryId) -> String {
        crate::freshness::initial_dream_pass_identity(owner, id)
    }

    pub(crate) fn was_processed(&self, id: MemoryId) -> bool {
        self.processing
            .state(id, crate::processing_epoch_model::DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
    }

    pub(crate) fn pending_pass_id_for_source(&self, source: MemoryId) -> Option<String> {
        self.journal
            .pending_for_source(source)
            .map(|pass| pass.pass_id)
    }

    pub(crate) fn is_settled(&self, pass_id: &str) -> bool {
        self.journal.is_settled(pass_id)
    }
}
