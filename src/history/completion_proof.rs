//! Read-only reconstruction of physical Import completion evidence.

use crate::archive_import_model::ImportReceipt;
use crate::freshness_storage::HistoricalImportFreshnessSettlement;
use crate::imports::epoch_codec;
use crate::imports::epoch_model::{ImportEpochStatus, ImportError, ImportStart};
use crate::imports::epoch_store::ImportEpochStore;
use crate::owner_lifecycle::{HistorySource, OwnerError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Eq, PartialEq)]
struct AcceptedSettlement {
    digest: String,
    global: u64,
    archive: u64,
    graph: u64,
    memory_graph: u64,
}

pub(crate) fn validate_import_binding(
    start: &ImportStart,
    settlement: &HistoricalImportFreshnessSettlement,
) -> Result<(), ImportError> {
    let scope = settlement.scope_identity().ok_or(ImportError::Epoch(
        "completion requires an epoch-scoped settlement",
    ))?;
    let start_digest: [u8; 32] = Sha256::digest(epoch_codec::encode_start(start)?).into();
    if settlement.owner != start.owner_uuid
        || id_text(&settlement.import_id) != start.import_id
        || scope.owner != start.owner_uuid
        || scope.import_id != settlement.import_id
        || scope.start_evidence_digest != start_digest
        || settlement.start_evidence_digest != start_digest
    {
        return Err(ImportError::Epoch(
            "completion settlement/start binding mismatch",
        ));
    }
    Ok(())
}

pub(crate) fn completion_epochs(
    source: &HistorySource<'_>,
    receipts: &BTreeMap<String, ImportReceipt>,
) -> Result<ImportEpochStore, OwnerError> {
    let mut epochs = ImportEpochStore::default();
    let mut settlements = BTreeMap::<String, AcceptedSettlement>::new();
    let mut cursor = source
        .cursor()
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let payload = frame.payload.as_slice();
        if payload.starts_with(b"CVAFRS06") {
            let settlement = HistoricalImportFreshnessSettlement::decode(payload)
                .map_err(OwnerError::Freshness)?;
            let id = id_text(&settlement.import_id);
            let start = match epochs.status(&id) {
                Some(ImportEpochStatus::Open(start))
                | Some(ImportEpochStatus::Completed { start, .. }) => Some(start),
                None if settlement.semantic.is_some() => continue,
                None => {
                    return Err(
                        ImportError::Epoch("scoped settlement lacks its import start").into(),
                    );
                }
            };
            if let Some(start) = start {
                validate_import_binding(&start, &settlement)?;
                let evidence = AcceptedSettlement {
                    digest: format!("{:x}", Sha256::digest(payload)),
                    global: settlement.global_version,
                    archive: settlement.archive_version,
                    graph: settlement.graph_version,
                    memory_graph: settlement.memory_graph_version,
                };
                if let Some(previous) = settlements.get(&id) {
                    if previous != &evidence {
                        return Err(
                            ImportError::Epoch("conflicting physical import settlements").into(),
                        );
                    }
                } else {
                    settlements.insert(id, evidence);
                }
            }
        }
        if let Some(epoch_codec::ImportEpochRecord::Complete(complete)) =
            epoch_codec::decode(payload)?
        {
            let settlement = settlements
                .get(&complete.import_id)
                .ok_or(ImportError::Epoch(
                    "completion lacks a preceding scoped settlement",
                ))?;
            if complete.historical_freshness_settlement_digest != settlement.digest
                || complete.post.global_version < settlement.global
                || complete.post.archive_version < settlement.archive
                || complete.post.graph_version < settlement.graph
                || complete.post.memory_graph_version < settlement.memory_graph
            {
                return Err(ImportError::Epoch("completion settlement evidence mismatch").into());
            }
        }
        epochs.ingest(payload)?;
    }
    epochs.validate_owner(source.owner_uuid())?;
    crate::owner_lifecycle::validate_import_receipts(&epochs, receipts)?;
    Ok(epochs)
}

fn id_text(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}
