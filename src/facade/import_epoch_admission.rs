//! Existing receipt-to-epoch validation above the two durable owners.
use crate::imports::{ImportEpochStatus, ImportEpochStore, ImportError};
use std::collections::BTreeMap;

/// Receipt authority is Archive-owned; compare it above the epoch store.
pub(crate) fn validate_receipts(
    epochs: &ImportEpochStore,
    receipts: &BTreeMap<String, crate::imports::ImportReceipt>,
) -> Result<(), ImportError> {
    for status in epochs.statuses() {
        if let ImportEpochStatus::Completed { start, complete } = status {
            let receipt = receipts
                .get(&start.import_key)
                .ok_or(ImportError::Epoch("completed import lacks receipt"))?;
            if crate::archive_import_model::receipt_digest(receipt)? != complete.receipt_digest {
                return Err(ImportError::Epoch("import receipt digest mismatch"));
            }
        }
    }
    Ok(())
}
