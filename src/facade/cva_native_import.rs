use crate::Cva;
use crate::imports::{ImportEpochStatus, ImportError, ImportReceipt, ImportStart, NativeImport};

impl Cva {
    /// Offline/exclusive native import. Mounted management must compose this through
    /// its exact REL mutation gate; this method never opens a second runtime.
    /// Retry the same normalized input after partial failure. A receipt acknowledges
    /// all source records, branch heads and Import-priority queue entries after sync.
    pub fn import_native(
        &mut self,
        input: &NativeImport,
        now_ns: i64,
    ) -> Result<ImportReceipt, ImportError> {
        self.runtime_import_native(input, now_ns)
    }

    #[cfg(test)]
    pub(crate) fn with_import_write_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.container.set_owner_write_scope(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        self.container.set_owner_write_scope(false);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub fn native_import_receipts(&self) -> Vec<ImportReceipt> {
        self.archive.import_receipts.values().cloned().collect()
    }

    pub fn open_import_epoch(&self) -> Option<ImportStart> {
        self.runtime_open_import_start()
    }

    pub fn import_epoch_status(&self, import_id: &str) -> Option<ImportEpochStatus> {
        self.runtime_import_epoch_status(import_id)
    }

    pub fn import_epoch_statuses(&self) -> Vec<ImportEpochStatus> {
        self.runtime_import_epoch_statuses()
    }
}
