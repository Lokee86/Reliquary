//! Compare Archive-owned receipt evidence to the retained Import epoch log.

use crate::imports::epoch_model::{ImportEpochStatus, ImportError};
use crate::imports::epoch_store::ImportEpochStore;
use std::collections::BTreeMap;

pub(crate) fn validate_import_receipts(
    epochs: &ImportEpochStore,
    receipts: &BTreeMap<String, crate::archive_import_model::ImportReceipt>,
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
