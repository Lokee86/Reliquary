//! Runtime bindings for Import-owned historical operations.
use crate::{CvaError, MemoryId, freshness::FreshnessRecord};
use std::collections::BTreeSet;

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_reconstruct_historical_freshness(
        &mut self,
        options: crate::HistoricalReconstructionOptions,
    ) -> Result<crate::HistoricalReconstructionReport, crate::HistoricalReconstructionError> {
        let (report, due) =
            crate::imports::historical_owner_operations::reconstruct(&mut self.owner, options)?;
        if let Some(due) = due {
            self.freshness_due = due;
            self.freshness_notifications.clear();
        }
        Ok(report)
    }

    pub(crate) fn runtime_freeze_historical_import(
        &self,
        requested_complete: bool,
    ) -> Result<crate::imports::historical_settlement_plan::FrozenHistoricalImport, CvaError> {
        crate::imports::historical_owner_operations::prepare(&self.owner, requested_complete)
            .map_err(CvaError::from)
    }

    pub(crate) fn runtime_commit_historical_import(
        &mut self,
        prepared: &crate::imports::historical_settlement_plan::PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        let Some(due) =
            crate::imports::historical_owner_operations::commit(&mut self.owner, prepared)?
        else {
            return Ok(false);
        };
        self.freshness_due = due;
        self.freshness_notifications.clear();
        Ok(true)
    }

    pub(crate) fn runtime_historical_import_base(
        &self,
        scope: &crate::imports::ImportEpochScope,
    ) -> Result<std::collections::BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        crate::imports::historical_owner_operations::historical_import_base(&self.owner, scope)
    }

    pub(crate) fn runtime_import_epoch_scope(
        &self,
    ) -> Result<crate::history::ImportEpochScope, CvaError> {
        crate::imports::historical_owner_operations::import_epoch_scope(&self.owner)
    }

    pub(crate) fn runtime_validate_semantic_import_scope(
        &self,
        settlement: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
        scope: &crate::freshness_storage::ImportSemanticScope,
    ) -> Result<(), CvaError> {
        crate::imports::historical_owner_operations::validate_semantic_scope(
            &self.owner,
            settlement,
            scope,
        )
    }

    pub(crate) fn runtime_historical_corpus_digest(
        &self,
        through: u64,
    ) -> Result<[u8; 32], CvaError> {
        crate::imports::historical_owner_operations::corpus_digest(&self.owner, through)
    }

    pub(crate) fn runtime_historical_scoped_corpus_digest(
        &self,
        source_nodes: &BTreeSet<(String, String)>,
    ) -> Result<[u8; 32], CvaError> {
        crate::imports::historical_owner_operations::scoped_corpus_digest(&self.owner, source_nodes)
    }

    pub(crate) fn runtime_historical_admission_proof(
        &self,
    ) -> Result<crate::history::import_admission::AdmissionProof, CvaError> {
        crate::imports::historical_owner_operations::admission_proof(&self.owner)
    }

    pub(crate) fn runtime_historical_admission_proof_scoped(
        &self,
        scope: Option<&crate::imports::ImportEpochScope>,
    ) -> Result<crate::history::import_admission::AdmissionProof, CvaError> {
        crate::imports::historical_owner_operations::admission_proof_scoped(&self.owner, scope)
    }

    pub(crate) fn runtime_has_accepted_historical_settlement(
        &self,
        import_id: Option<&str>,
    ) -> bool {
        crate::imports::historical_owner_operations::has_accepted_settlement(&self.owner, import_id)
    }
}
