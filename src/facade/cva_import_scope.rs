//! Store-derived attribution for one open native import epoch.
use crate::{Cva, CvaError, history::ImportEpochScope};

impl Cva {
    pub(crate) fn validate_semantic_import_scope(
        &mut self,
        settlement: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
        scope: &crate::freshness_storage::ImportSemanticScope,
    ) -> Result<(), CvaError> {
        self.runtime_validate_semantic_import_scope(settlement, scope)
    }

    pub(crate) fn import_epoch_scope(&mut self) -> Result<ImportEpochScope, CvaError> {
        self.runtime_import_epoch_scope()
    }
}

#[cfg(test)]
#[path = "../imports/epoch_scope_tests.rs"]
mod tests;
