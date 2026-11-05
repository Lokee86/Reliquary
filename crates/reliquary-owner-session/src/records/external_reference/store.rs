use super::codec::{MAGIC, decode, encode, make_receipt};
use super::model::{
    ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt, MAX_PRINCIPAL_BYTES,
};
use crate::records::external_index::model::{
    ExternalSourceError, MAX_EXTERNAL_INDEX_BYTES, identifier,
};
use reliquary_container::Container;
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Default)]
pub(crate) struct ExternalReferenceStore {
    owner_id: Option<[u8; 16]>,
    snapshots: BTreeMap<(String, String), ExternalReferenceSnapshotReceipt>,
}

impl ExternalReferenceStore {
    pub(crate) fn open(container: &Container) -> Result<Self, String> {
        if container.identity().file_kind != reliquary_container::FileKind::Reliquary {
            return Err("external reference receipts require a REL owner".into());
        }
        let owner_id = container.owner_id();
        let mut store = Self {
            owner_id: Some(owner_id),
            ..Self::default()
        };
        let mut cursor = container
            .physical_cursor()
            .map_err(|error| error.to_string())?;
        while let Some(mut frame) = cursor.next_frame().map_err(|error| error.to_string())? {
            let length = frame.object().length;
            if length < MAGIC.len() as u64 {
                continue;
            }
            let mut prefix = [0; 8];
            frame
                .read_exact(&mut prefix)
                .map_err(|error| error.to_string())?;
            if &prefix != MAGIC {
                continue;
            }
            if length > MAX_EXTERNAL_INDEX_BYTES as u64 {
                return Err("external reference receipt exceeds its size limit".into());
            }
            let mut payload = Vec::with_capacity(length as usize);
            payload.extend_from_slice(&prefix);
            let remainder = usize::try_from(length - MAGIC.len() as u64)
                .map_err(|_| "external reference receipt size overflow")?;
            payload.resize(length as usize, 0);
            frame
                .read_exact(&mut payload[MAGIC.len()..MAGIC.len() + remainder])
                .map_err(|error| error.to_string())?;
            let receipt = decode(&payload).map_err(|error| error.to_string())?;
            if receipt.owner_id != owner_id {
                return Err("external reference receipt belongs to another REL".into());
            }
            let key = (
                receipt.manifest.namespace.clone(),
                receipt.authorizing_principal.clone(),
            );
            if let Some(previous) = store.snapshots.get(&key) {
                let previous_watermark = previous.manifest.watermark;
                let next_watermark = receipt.manifest.watermark;
                if next_watermark < previous_watermark
                    || (next_watermark == previous_watermark && previous != &receipt)
                {
                    return Err("conflicting external reference snapshot history".into());
                }
            }
            store.snapshots.insert(key, receipt);
        }
        Ok(store)
    }

    pub(crate) fn get(
        &self,
        namespace: &str,
        principal: &str,
    ) -> Option<&ExternalReferenceSnapshotReceipt> {
        self.snapshots
            .get(&(namespace.to_owned(), principal.to_owned()))
    }

    pub(crate) fn record(
        &mut self,
        container: &mut Container,
        principal: &str,
        manifest: ExternalReferenceSnapshotManifest,
    ) -> Result<ExternalReferenceSnapshotReceipt, ExternalSourceError> {
        if self.owner_id != Some(container.owner_id()) {
            return Err(ExternalSourceError::Unauthorized);
        }
        identifier(principal)?;
        if principal.len() > MAX_PRINCIPAL_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
        manifest.validate()?;
        let receipt = make_receipt(container.owner_id(), principal, manifest)?;
        let key = (
            receipt.manifest.namespace.clone(),
            receipt.authorizing_principal.clone(),
        );
        if let Some(previous) = self.snapshots.get(&key) {
            if previous == &receipt {
                return Ok(previous.clone());
            }
            if receipt.manifest.watermark <= previous.manifest.watermark {
                return Err(ExternalSourceError::Conflict);
            }
        }
        let bytes = encode(&receipt)?;
        if container.append(&bytes).is_err() || container.sync().is_err() {
            self.snapshots.clear();
            return Err(ExternalSourceError::Storage);
        }
        self.snapshots.insert(key, receipt.clone());
        Ok(receipt)
    }
}
