//! Exclusive import completion composition; stores retain their own evidence.
use super::{
    ImportComplete, ImportCompletionOutcome, ImportCompletionPending, ImportEpochStatus,
    ImportEpochStore, ImportError, ImportStart, epoch_codec, prepare,
};
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::{Container, Cva, CvaError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn epoch_error(message: &'static str) -> CvaError {
    CvaError::Import(ImportError::Epoch(message))
}

fn id_text(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn settlement_digest(settlement: &HistoricalImportFreshnessSettlement) -> String {
    format!("{:x}", Sha256::digest(settlement.encode()))
}

#[derive(Eq, PartialEq)]
struct AcceptedSettlement {
    digest: String,
    global: u64,
    archive: u64,
    graph: u64,
    memory_graph: u64,
}

fn validate_binding(
    start: &ImportStart,
    settlement: &HistoricalImportFreshnessSettlement,
) -> Result<(), CvaError> {
    let scope = settlement
        .epoch
        .as_ref()
        .ok_or_else(|| epoch_error("completion requires an epoch-scoped settlement"))?;
    let start_digest: [u8; 32] = Sha256::digest(epoch_codec::encode_start(start)?).into();
    if settlement.owner != start.owner_uuid
        || id_text(&settlement.import_id) != start.import_id
        || scope.owner != start.owner_uuid
        || scope.import_id != settlement.import_id
        || scope.start_evidence_digest != start_digest
        || settlement.start_evidence_digest != start_digest
    {
        return Err(epoch_error("completion settlement/start binding mismatch"));
    }
    Ok(())
}

impl Cva {
    /// Explicit offline/exclusive completion. Call after historical settlement.
    /// No caller-supplied digests or readiness assertions authorize publication.
    /// Until attributed readiness is integrated, only empty imports can complete;
    /// all nonempty receipts return Pending and retain the ordinary-write fence.
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
        // This is a vacuous readiness proof only: no imported conversation,
        // source, Episode, birth or downstream work exists. Step 29 replaces
        // this conservative gate with owner-derived attributed readiness.
        if !receipt.conversations.is_empty() {
            return Ok(ImportCompletionOutcome::Pending(
                ImportCompletionPending::AttributedReadinessUnavailable,
            ));
        }
        let Some(settlement) = self
            .freshness
            .import_settlements()
            .iter()
            .find(|settlement| id_text(&settlement.import_id) == start.import_id)
            .cloned()
        else {
            return Ok(ImportCompletionOutcome::Pending(
                ImportCompletionPending::MissingFreshnessSettlement,
            ));
        };
        validate_binding(&start, &settlement)?;
        let scope = settlement.epoch.as_ref().expect("validated epoch binding");
        if !scope.source_nodes.is_empty()
            || !scope.born_memories.is_empty()
            || !settlement.cohort.is_empty()
            || !settlement.records.is_empty()
            || !settlement.superseded.is_empty()
            || settlement.base_cut != settlement.settlement_cut
        {
            return Err(epoch_error("empty import has nonempty settlement effects"));
        }
        let post = self.import_watermarks();
        if settlement.settlement_cut != self.rel_turn_count()
            || settlement.global_version != post.global_version
            || settlement.archive_version != post.archive_version
            || settlement.graph_version != post.graph_version
            || settlement.memory_graph_version != post.memory_graph_version
        {
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
            epochs.append_complete(&mut cva.container, complete.clone())
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
    fn completion_epochs_from_bytes(&self) -> Result<ImportEpochStore, CvaError> {
        let mut epochs = ImportEpochStore::default();
        let mut settlements = BTreeMap::new();
        let mut scan = Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads::<CvaError>(|_, payload| {
            if payload.starts_with(b"CVAFRS05") {
                let settlement = HistoricalImportFreshnessSettlement::decode(payload)
                    .map_err(CvaError::Freshness)?;
                let id = id_text(&settlement.import_id);
                let start = match epochs.status(&id) {
                    Some(ImportEpochStatus::Open(start))
                    | Some(ImportEpochStatus::Completed { start, .. }) => start,
                    None => return Err(epoch_error("scoped settlement lacks its import start")),
                };
                validate_binding(&start, &settlement)?;
                // Retain only small completion evidence, not a second map of
                // every historical sparse replacement population.
                let evidence = AcceptedSettlement {
                    digest: format!("{:x}", Sha256::digest(payload)),
                    global: settlement.global_version,
                    archive: settlement.archive_version,
                    graph: settlement.graph_version,
                    memory_graph: settlement.memory_graph_version,
                };
                if let Some(previous) = settlements.get(&id) {
                    if previous != &evidence {
                        return Err(epoch_error("conflicting physical import settlements"));
                    }
                } else {
                    settlements.insert(id, evidence);
                }
            }
            if let Some(epoch_codec::ImportEpochRecord::Complete(complete)) =
                epoch_codec::decode(payload)?
            {
                let settlement = settlements
                    .get(&complete.import_id)
                    .ok_or_else(|| epoch_error("completion lacks a preceding scoped settlement"))?;
                if complete.historical_freshness_settlement_digest != settlement.digest
                    || complete.post.global_version < settlement.global
                    || complete.post.archive_version < settlement.archive
                    || complete.post.graph_version < settlement.graph
                    || complete.post.memory_graph_version < settlement.memory_graph
                {
                    return Err(epoch_error("completion settlement evidence mismatch"));
                }
            }
            epochs.ingest(payload)?;
            Ok(())
        })?;
        epochs.validate_owner(self.owner_uuid())?;
        epochs.validate_receipts(&self.archive.import_receipts)?;
        Ok(epochs)
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
