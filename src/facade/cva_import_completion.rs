//! Thin Cva adapter for physical import recovery and owner fact gathering.
use crate::imports::{ImportCompletionOutcome, ImportEpochStore};
use crate::{Cva, CvaError};

impl Cva {
    /// Exclusive offline completion. Import owns readiness and publication
    /// policy; this adapter supplies accepted owner facts in checkpoint order.
    pub fn complete_native_import(
        &mut self,
        import_id: &str,
        now_ns: i64,
    ) -> Result<ImportCompletionOutcome, CvaError> {
        self.validate_historical_import_settlements()?;
        let mut session = crate::owner_lifecycle::ImportSession::offline_rel(&mut self.container)?;
        let (mut epochs, recovered) = crate::imports::completion_coordinator::recover_import(
            &mut session,
            &self.archive,
            import_id,
        )?;
        let start = match recovered {
            crate::imports::completion_coordinator::RecoveredImport::Completed(outcome) => {
                self.import_epochs = epochs;
                return Ok(outcome);
            }
            crate::imports::completion_coordinator::RecoveredImport::Open(start) => start,
        };
        self.import_epochs = epochs.clone();
        drop(session);
        crate::imports::completion_coordinator::validate_started_at(&start, now_ns)?;
        let Some(receipt) = self.archive.import_receipts.get(&start.import_key).cloned() else {
            return Ok(crate::imports::completion_coordinator::missing_receipt());
        };
        crate::imports::completion_coordinator::validate_receipt(&start, &receipt)?;
        let settlement = self
            .freshness
            .import_settlements()
            .iter()
            .find(|settlement| {
                settlement
                    .import_id
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == start.import_id
            })
            .cloned();
        let scope = self.import_epoch_scope()?;
        let current_watermarks = self.import_watermarks();
        let current_turn = self.rel_turn_count();
        let insomnia = crate::insomnia::import_readiness::ImportReadiness::new(&self.insomnia);
        let dream = crate::dream_readiness::DreamReadiness::new(
            &self.dream_freshness,
            &self.processing_epochs,
        );
        let perception = crate::perception_import_readiness::ImportReadiness::new(
            &self.memories,
            &self.entity_resolutions,
        );
        let mut session = crate::owner_lifecycle::ImportSession::offline_rel(&mut self.container)?;
        let outcome = crate::imports::completion_coordinator::complete_import(
            &mut session,
            &mut epochs,
            &start,
            &receipt,
            settlement.as_ref(),
            &scope,
            &self.archive,
            &self.memories,
            &insomnia,
            &dream,
            &perception,
            current_turn,
            current_watermarks,
            now_ns,
        )?;
        self.import_epochs = epochs;
        Ok(outcome)
    }

    /// Rebuild exact accepted epoch state from the bounded REL history source.
    pub(crate) fn completion_epochs_from_bytes(&self) -> Result<ImportEpochStore, CvaError> {
        let access = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        crate::imports::completion_coordinator::recover_epochs(&access, &self.archive)
    }
}
