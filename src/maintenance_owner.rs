use super::*;

impl Rel {
    pub fn conversations(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<app::ManagementPage<app::ConversationInspection>, ProductError> {
        app::management_conversations(self.session(), offset, limit)
            .map_err(|error| ProductError::new(ProductErrorKind::InvalidInput, error.to_string()))
    }
    pub fn current_historical_proof(&self) -> Result<HistoricalProofSummary, ProductError> {
        let proof = self
            .session()
            .historical_proof(
                self.owner_id().as_bytes(),
                &self.session().current_historical_cut(),
            )
            .map_err(|error| ProductError::new(ProductErrorKind::Unavailable, error.to_string()))?;
        Ok(proof_summary(proof))
    }
    pub fn plan_rewrite(&self, policy: RewritePolicy) -> Result<RewritePlan, ProductError> {
        app::plan_rel_rewrite(self.path(), self.session(), policy).map_err(rewrite_error)
    }
    pub fn memory_history(&self, memory_id: MemoryId) -> Result<MemoryHistoryLookup, ProductError> {
        app::read_owner_memory_history(
            &app::EntityOwnerSession::Rel(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
    pub fn memory_history_at_cut(
        &self,
        memory_id: MemoryId,
        memory_version: u64,
    ) -> Result<MemoryHistoryLookup, ProductError> {
        app::read_owner_memory_history_at_cut(
            &app::EntityOwnerSession::Rel(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
            memory_version,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
    pub fn prepare_memory_prune(
        &self,
        memory_id: MemoryId,
    ) -> Result<MemoryMaintenancePreparation, ProductError> {
        app::prepare_tombstoned_memory_maintenance(
            &app::EntityOwnerSession::Rel(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
}

impl Phylactery {
    pub fn current_historical_proof(&self) -> Result<HistoricalProofSummary, ProductError> {
        let proof = self
            .session()
            .historical_proof(
                self.owner_id().as_bytes(),
                self.session().current_historical_cut(),
            )
            .map_err(|error| ProductError::new(ProductErrorKind::Unavailable, error.to_string()))?;
        Ok(proof_summary(proof))
    }
    pub fn plan_rewrite(&self, policy: RewritePolicy) -> Result<RewritePlan, ProductError> {
        app::plan_phy_rewrite(self.path(), self.session(), policy).map_err(rewrite_error)
    }
    pub fn memory_history(&self, memory_id: MemoryId) -> Result<MemoryHistoryLookup, ProductError> {
        app::read_owner_memory_history(
            &app::EntityOwnerSession::Phy(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
    pub fn memory_history_at_cut(
        &self,
        memory_id: MemoryId,
        memory_version: u64,
    ) -> Result<MemoryHistoryLookup, ProductError> {
        app::read_owner_memory_history_at_cut(
            &app::EntityOwnerSession::Phy(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
            memory_version,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
    pub fn prepare_memory_prune(
        &self,
        memory_id: MemoryId,
    ) -> Result<MemoryMaintenancePreparation, ProductError> {
        app::prepare_tombstoned_memory_maintenance(
            &app::EntityOwnerSession::Phy(self.session()),
            self.owner_id().as_bytes(),
            memory_id,
        )
        .map_err(|error| ProductError::new(ProductErrorKind::Verification, error.to_string()))
    }
}
