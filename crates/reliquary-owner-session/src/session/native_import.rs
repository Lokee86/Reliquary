//! Owner-derived native import epoch admission and retry identity.

use super::*;
use crate::records::import::is_import_epoch_marker;
use crate::records::{
    ImportEpochStatus, ImportFreshnessSettlement, ImportStart, ImportWatermarks, import_id,
};
use std::borrow::Cow;

#[path = "native_import_completion.rs"]
mod completion;
#[path = "native_import_freshness.rs"]
mod freshness;
#[path = "native_import_plan.rs"]
mod plan;
#[path = "native_import_postprocessing.rs"]
mod postprocessing;
pub(super) use postprocessing::{
    VerifiedDreamImport, native_dream_episode_is_mapped, native_import_dream_prefix,
};
#[path = "native_import_work.rs"]
mod work;
pub use work::InsomniaEpisodeStatus;
pub(super) use work::{ActiveImportSource, ImportSourceMap};
#[path = "native_import_writer.rs"]
mod writer;

impl RelSession {
    /// Current durable epoch, including an interrupted import with no receipt.
    pub fn open_native_import_id(&self) -> Option<&str> {
        self.imports
            .open_epoch()
            .map(|start| start.import_id.as_str())
    }

    pub fn native_import_receipt(
        &self,
        import_key: &[u8; 32],
    ) -> Result<Option<Cow<'_, crate::model::ImportReceipt>>, OwnerSessionError> {
        Ok(self.archive.import_receipt(import_key)?)
    }

    pub fn native_import_receipt_digest(&self, import_key: &[u8; 32]) -> Option<[u8; 32]> {
        self.archive
            .import_receipt_identity(import_key)
            .map(|identity| identity.receipt_digest)
    }

    pub fn native_import_settlement(&self, import_id: &str) -> Option<&ImportFreshnessSettlement> {
        self.imports.settlement(import_id)
    }
}

pub(super) fn validate_native_import_receipts(
    container: &Container,
    imports: &ImportEpochStore,
    archive: &Archive,
    receipt_positions: &[(u64, [u8; 32])],
) -> Result<(), OwnerSessionError> {
    let mut prefix = ImportEpochStore::prefix(container.owner_id());
    let mut received = std::collections::BTreeSet::new();
    let mut rewind_heads = std::collections::BTreeSet::new();
    // Archive has already validated every receipt, including repeated raw
    // frames. Check their positions against the epoch and source changes.
    let mut receipts = receipt_positions.iter().peekable();
    let mut markers = container.scan_matching_frames(|payload| {
        is_import_epoch_marker(payload)
            || reliquary_archive::copied_source_change_candidate(payload)
    })?;
    while let Some(marker) = markers.next_frame()? {
        while let Some((_, key)) = receipts.next_if(|(offset, _)| *offset < marker.object.offset) {
            received.insert(validate_receipt_at_epoch(&prefix, imports, archive, key)?);
        }
        if !is_import_epoch_marker(marker.payload) {
            if let Some(change) = reliquary_archive::copied_source_change_in_frame(marker.payload)?
            {
                let head = change.request.source_sha256;
                let key = head
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let id = import_id(container.owner_id(), &key);
                if prefix.open_epoch().is_some()
                    || !(rewind_heads.contains(&head)
                        || (received.contains(&id)
                            && matches!(
                                prefix.status(&id),
                                Some(ImportEpochStatus::Completed { .. })
                            )))
                {
                    return Err(OwnerSessionError::Record(
                        "copied source change has no completed owner head at its physical position"
                            .into(),
                    ));
                }
                rewind_heads.extend(change.receipt.resulting_head_sha256);
            }
            continue;
        }
        if marker
            .payload
            .starts_with(crate::records::IMPORT_PROCESSING_MAGIC)
            && !received.contains(
                &crate::records::processing_marker_id(marker.payload)
                    .map_err(OwnerSessionError::Record)?,
            )
        {
            return Err(OwnerSessionError::Record(
                "import processing plan precedes its Archive receipt".into(),
            ));
        }
        prefix
            .ingest_prefix(marker.payload)
            .map_err(OwnerSessionError::Record)?;
    }
    for (_, key) in receipts {
        validate_receipt_at_epoch(&prefix, imports, archive, key)?;
    }
    Ok(())
}

fn validate_receipt_at_epoch(
    prefix: &ImportEpochStore,
    imports: &ImportEpochStore,
    archive: &Archive,
    key: &[u8; 32],
) -> Result<String, OwnerSessionError> {
    let receipt = archive
        .import_receipt_identity(key)
        .ok_or_else(|| OwnerSessionError::Record("missing Archive import receipt".into()))?;
    let key = key
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let id = import_id(receipt.owner_uuid, &key);
    if prefix
        .open_epoch()
        .is_none_or(|start| start.import_id != id)
    {
        return Err(OwnerSessionError::Record(
            "Archive import receipt has no owner epoch at its physical position".into(),
        ));
    }
    let (start, completion) = match imports.status(&id) {
        Some(ImportEpochStatus::Open(start)) => (start, None),
        Some(ImportEpochStatus::Completed { start, complete }) => (start, Some(complete)),
        None => {
            return Err(OwnerSessionError::Record(
                "Archive import receipt has no owner epoch".into(),
            ));
        }
    };
    if start.format_version != 2
        || start.import_key != key
        || start.source_namespace != receipt.source_namespace
        || start.source_format != receipt.source_format
    {
        return Err(OwnerSessionError::Record(
            "Archive import receipt differs from its owner epoch".into(),
        ));
    }
    if let Some(completion) = completion {
        let digest = receipt
            .receipt_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if completion.receipt_digest != digest {
            return Err(OwnerSessionError::Record(
                "Archive import receipt differs from its completion".into(),
            ));
        }
    }
    Ok(id)
}

pub(super) fn decode_import_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut key = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        key[index] = (high << 4) | low;
    }
    Some(key)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "native_import_tests.rs"]
mod tests;
