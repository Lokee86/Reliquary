use super::{ImportComplete, ImportEpochStatus, ImportError, ImportReceipt, ImportStart};
use reliquary_container::Container;
use std::collections::BTreeMap;

pub(crate) use reliquary_import_store::import_id;
#[derive(Clone, Default)]
pub(crate) struct ImportEpochStore(reliquary_import_store::ImportEpochStore);
impl ImportEpochStore {
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), ImportError> {
        Ok(self.0.ingest(payload)?)
    }
    pub(crate) fn append_start(
        &mut self,
        container: &mut Container,
        start: ImportStart,
    ) -> Result<(), ImportError> {
        Ok(self.0.append_start(container, start)?)
    }
    pub(crate) fn open(&self) -> Option<&ImportStart> {
        self.0.open()
    }
    pub(crate) fn status(&self, id: &str) -> Option<ImportEpochStatus> {
        self.0.status(id)
    }
    pub(crate) fn statuses(&self) -> Vec<ImportEpochStatus> {
        self.0.statuses()
    }
    pub(crate) fn validate_owner(&self, owner: Option<[u8; 16]>) -> Result<(), ImportError> {
        Ok(self.0.validate_owner(owner)?)
    }
    pub(crate) fn validate_receipts(
        &self,
        receipts: &BTreeMap<String, ImportReceipt>,
    ) -> Result<(), ImportError> {
        // Preserve missing-receipt precedence by only hashing receipts used by completed epochs.
        let mut digests = BTreeMap::new();
        for status in self.statuses() {
            if let ImportEpochStatus::Completed { start, .. } = status {
                if let Some(receipt) = receipts.get(&start.import_key) {
                    digests.insert(start.import_key, super::prepare::digest(receipt)?);
                }
            }
        }
        Ok(self.0.validate_receipts(&digests)?)
    }
    pub(super) fn check_complete(&self, complete: &ImportComplete) -> Result<bool, ImportError> {
        Ok(self.0.check_complete(complete)?)
    }
    #[cfg(test)]
    pub(super) fn append_complete(
        &mut self,
        container: &mut Container,
        complete: ImportComplete,
    ) -> Result<(), ImportError> {
        if self.check_complete(&complete)? {
            let bytes = super::epoch_codec::encode_complete(&complete)?;
            container.append(&bytes)?;
            self.ingest(&bytes)?;
        }
        Ok(())
    }
}
