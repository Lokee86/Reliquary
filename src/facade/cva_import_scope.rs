//! Store-derived attribution for one open native import epoch.
use crate::imports::ImportStart;
use crate::{Cva, CvaError, history::ImportEpochScope};

impl Cva {
    pub(crate) fn validate_semantic_import_scope(
        &mut self,
        settlement: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
        scope: &crate::freshness_storage::ImportSemanticScope,
    ) -> Result<(), CvaError> {
        let prepared = crate::history::prepare_semantic_import_scope(
            self.owner_uuid(),
            &self.archive,
            &self.import_epochs,
            settlement,
            scope,
        )?;
        match prepared {
            crate::history::PreparedSemanticImportScope::InitialHistory => Ok(()),
            crate::history::PreparedSemanticImportScope::Framed(facts) => {
                let source = crate::owner_lifecycle::HistorySource::new(&self.container)
                    .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
                crate::history::validate_semantic_import_births(
                    &source,
                    &self.archive,
                    &self.memories,
                    facts,
                )
            }
        }
    }

    pub(crate) fn import_epoch_scope(&mut self) -> Result<ImportEpochScope, CvaError> {
        let start = self.open_import_epoch().ok_or_else(|| {
            CvaError::Freshness("historical reconstruction requires an open import epoch".into())
        })?;
        self.import_epoch_scope_for(start)
    }

    fn import_epoch_scope_for(&mut self, start: ImportStart) -> Result<ImportEpochScope, CvaError> {
        crate::history::read_import_epoch_scope(
            &crate::owner_lifecycle::HistoryReadAccess::new(&self.container),
            &self.archive,
            &self.memories,
            start,
        )
    }
}

#[cfg(test)]
#[path = "../imports/epoch_scope_tests.rs"]
mod tests;
