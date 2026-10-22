//! Exclusive import completion composition; stores retain their own evidence.

use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::imports::{
    ImportComplete, ImportCompletionOutcome, ImportCompletionPending, ImportEpochStatus,
    ImportEpochStore, ImportError, prepare,
};
use crate::{Container, Cva, CvaError};
use sha2::{Digest, Sha256};

fn epoch_error(message: &'static str) -> CvaError {
    CvaError::Import(ImportError::Epoch(message))
}

fn id_text(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn settlement_digest(settlement: &HistoricalImportFreshnessSettlement) -> String {
    format!("{:x}", Sha256::digest(settlement.encode()))
}

pub(in crate::imports) fn settlement_is_current(
    cva: &Cva,
    settlement: &HistoricalImportFreshnessSettlement,
) -> bool {
    let post = cva.import_watermarks();
    settlement.settlement_cut == cva.rel_turn_count()
        && settlement.global_version == post.global_version
        && settlement.archive_version == post.archive_version
        && settlement.graph_version == post.graph_version
        && settlement.memory_graph_version == post.memory_graph_version
}

impl Cva {
    /// Explicit offline/exclusive completion. Call after historical settlement.
    /// No caller-supplied digests or readiness assertions authorize publication.
    /// The coordinator derives every readiness decision from owner evidence.
    pub fn complete_native_import(
        &mut self,
        import_id: &str,
        now_ns: i64,
    ) -> Result<ImportCompletionOutcome, CvaError> {
        self.validate_historical_import_settlements()?;
        // Read the accepted physical markers on every attempt. In particular,
        // append may have succeeded before an earlier synchronization failed.
        let mut epochs = self.completion_epochs_from_bytes()?;
        let status = epochs
            .status(import_id)
            .ok_or_else(|| epoch_error("unknown import completion identity"))?;
        let start = match status {
            ImportEpochStatus::Completed { complete, .. } => {
                self.container.sync()?;
                let open = epochs.open().is_some();
                self.import_epochs = epochs;
                self.container.set_write_admission_closed(open);
                return Ok(ImportCompletionOutcome::Completed {
                    complete,
                    committed: false,
                });
            }
            ImportEpochStatus::Open(start) => start,
        };
        // A scan may recover a start accepted before a prior caller lost its
        // in-memory update. Pending/error paths must retain that durable fence.
        self.import_epochs = epochs.clone();
        self.container.set_write_admission_closed(true);
        if now_ns < start.started_at_ns {
            return Err(epoch_error("import completion precedes its start"));
        }
        let Some(receipt) = self.archive.import_receipts.get(&start.import_key).cloned() else {
            return Ok(ImportCompletionOutcome::Pending(
                ImportCompletionPending::MissingReceipt,
            ));
        };
        if receipt.source_namespace != start.source_namespace
            || receipt.source_format != start.source_format
        {
            return Err(epoch_error("completion receipt/start identity mismatch"));
        }
        // Derive scoped owner work first. Missing or failed imported work
        // remains observable even before historical freshness has settled.
        let settlement = self
            .freshness
            .import_settlements()
            .iter()
            .find(|settlement| id_text(&settlement.import_id) == start.import_id)
            .cloned();
        if let Some(blocker) =
            self.derive_import_readiness(&start, &receipt, settlement.as_ref())?
        {
            return Ok(ImportCompletionOutcome::Pending(
                ImportCompletionPending::Readiness(blocker),
            ));
        }
        let Some(settlement) = settlement else {
            return Ok(ImportCompletionOutcome::Pending(
                ImportCompletionPending::MissingFreshnessSettlement,
            ));
        };
        crate::history::validate_import_binding(&start, &settlement)?;
        let post = self.import_watermarks();
        if !settlement_is_current(self, &settlement) {
            return Err(epoch_error("completion settlement cut is stale"));
        }
        let complete = ImportComplete {
            owner_uuid: start.owner_uuid,
            import_id: start.import_id.clone(),
            start_digest: prepare::digest(&start)?,
            receipt_digest: prepare::digest(&receipt)?,
            completed_at_ns: now_ns,
            post,
            historical_freshness_settlement_digest: settlement_digest(&settlement),
        };
        let result = self.with_import_write_scope(|cva| {
            epochs.append_complete_coordinated(&mut cva.container, complete.clone())
        });
        // Store append changes its own local state and admission. Keep the
        // original owner state and fence until sync-before-acknowledgement.
        self.container.set_write_admission_closed(true);
        result?;
        self.container.sync()?;
        self.import_epochs = epochs;
        self.container.set_write_admission_closed(false);
        Ok(ImportCompletionOutcome::Completed {
            complete,
            committed: true,
        })
    }

    /// Rebuild validates physical settlement-before-completion ordering as well
    /// as exact accepted evidence. This also recovers uncertain append retries.
    pub(in crate::imports) fn completion_epochs_from_bytes(
        &self,
    ) -> Result<ImportEpochStore, CvaError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        crate::history::completion_epochs(&source, &self.archive.import_receipts)
    }

    pub(crate) fn validate_import_completion_settlements(&self) -> Result<(), CvaError> {
        if !self
            .import_epochs
            .statuses()
            .iter()
            .any(|status| matches!(status, ImportEpochStatus::Completed { .. }))
        {
            return Ok(());
        }
        self.completion_epochs_from_bytes().map(|_| ())
    }
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
