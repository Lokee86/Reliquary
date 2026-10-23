use crate::imports::ImportReadinessReport;
use crate::{Cva, CvaError};

impl Cva {
    /// Inspect owner-derived readiness for the exact currently open native import.
    /// This adapter gathers concrete owner facts; Import owns the readiness policy.
    pub fn native_import_readiness(
        &mut self,
        import_id: &str,
    ) -> Result<ImportReadinessReport, CvaError> {
        let epochs = self.completion_epochs_from_bytes()?;
        let start = crate::imports::readiness::requested_start(&epochs, import_id)?;
        let Some(receipt) = self.archive.import_receipts.get(&start.import_key).cloned() else {
            return Ok(crate::imports::readiness::missing_receipt_report(
                start.import_id,
            ));
        };

        let scope = self.import_epoch_scope()?;
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
        // Preserve the former second scope replay inside the readiness policy.
        let policy_scope = self.import_epoch_scope()?;
        let read_access = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        let insomnia = crate::insomnia::import_readiness::ImportReadiness::new(&self.insomnia);
        let dream = crate::dream_readiness::DreamReadiness::new(
            &self.dream_freshness,
            &self.processing_epochs,
        );
        let perception = crate::perception_import_readiness::ImportReadiness::new(
            &self.memories,
            &self.entity_resolutions,
        );
        let readiness = crate::imports::readiness::derive_import_readiness(
            &start,
            &receipt,
            settlement.as_ref(),
            &policy_scope,
            &self.archive,
            &self.memories,
            &insomnia,
            &dream,
            &perception,
            &read_access,
            self.rel_turn_count(),
            self.import_watermarks(),
        )?;
        Ok(crate::imports::readiness::readiness_report(
            start,
            settlement.as_ref(),
            &scope,
            readiness,
        ))
    }
}
