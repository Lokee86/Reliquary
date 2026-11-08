//! Owner-derived native import epoch admission and retry identity.

use super::*;
use crate::records::{
    ImportEpochStatus, ImportFreshnessSettlement, ImportStart, ImportWatermarks, import_id,
};

#[path = "native_import_completion.rs"]
mod completion;
#[path = "native_import_freshness.rs"]
mod freshness;
#[path = "native_import_plan.rs"]
mod plan;
#[path = "native_import_postprocessing.rs"]
mod postprocessing;
pub(super) use postprocessing::native_import_dream_prefix;
#[path = "native_import_work.rs"]
mod work;
pub use work::InsomniaEpisodeStatus;
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
    ) -> Option<&crate::model::ImportReceipt> {
        self.archive.import_receipt(import_key)
    }

    pub fn native_import_settlement(&self, import_id: &str) -> Option<&ImportFreshnessSettlement> {
        self.imports.settlement(import_id)
    }
}

pub(super) fn is_import_epoch_marker(payload: &[u8]) -> bool {
    matches!(
        payload.get(..8),
        Some(b"RELIEPS1" | b"RELIEPC1" | b"RELIEPS2" | b"RELIEPC2" | b"RELIFRS1" | b"RELIPLN1")
    )
}

pub(super) fn validate_native_import_receipts(
    container: &Container,
    imports: &ImportEpochStore,
) -> Result<(), OwnerSessionError> {
    let mut prefix = ImportEpochStore::prefix(container.owner_id());
    let mut received = std::collections::BTreeSet::new();
    let mut cursor = container.physical_cursor()?;
    while let Some(mut frame) = cursor.next_frame()? {
        use std::io::Read;
        let mut magic = [0u8; 8];
        if frame.object().length < 8 {
            continue;
        }
        frame
            .read_exact(&mut magic)
            .map_err(reliquary_container::ContainerError::from)?;
        let epoch_marker = is_import_epoch_marker(&magic);
        if !epoch_marker && &magic != b"CVAIMPT1" {
            continue;
        }
        let mut payload = magic.to_vec();
        frame
            .read_to_end(&mut payload)
            .map_err(reliquary_container::ContainerError::from)?;
        if epoch_marker {
            if &magic == crate::records::IMPORT_PROCESSING_MAGIC
                && !received.contains(
                    &crate::records::processing_marker_id(&payload)
                        .map_err(OwnerSessionError::Record)?,
                )
            {
                return Err(OwnerSessionError::Record(
                    "import processing plan precedes its Archive receipt".into(),
                ));
            }
            prefix
                .ingest_prefix(&payload)
                .map_err(OwnerSessionError::Record)?;
            continue;
        }
        let receipt = reliquary_archive::import_receipt_in_frame(&payload)?
            .ok_or_else(|| OwnerSessionError::Record("missing Archive import receipt".into()))?;
        let key = receipt
            .import_key
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
        received.insert(id);
    }
    Ok(())
}

fn decode_import_key(value: &str) -> Option<[u8; 32]> {
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
