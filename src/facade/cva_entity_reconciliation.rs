use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::{
    Cva, EntityReconciliationError, EntityReconciliationReport, GeneralEndpoint, Phylactery,
};

impl Cva {
    pub fn reconcile_entities<E: GeneralEndpoint>(
        &mut self,
        endpoint: &E,
        now_ns: i64,
    ) -> Result<EntityReconciliationReport, EntityReconciliationError> {
        crate::perception_entity_reconciliation_runtime::reconcile_rel(self, endpoint, now_ns)
    }

    pub(crate) fn prepare_entity_reconciliation(
        &self,
    ) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
        self.prepare_entity_reconciliation_runtime()
    }

    pub(crate) fn commit_entity_reconciliation(
        &mut self,
        prepared: EntityReconciliationPrepared,
        evaluation: EntityReconciliationEvaluation,
        now_ns: i64,
    ) -> Result<
        Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
        EntityReconciliationError,
    > {
        self.commit_entity_reconciliation_runtime(prepared, evaluation, now_ns)
    }
}

impl Phylactery {
    pub fn reconcile_entities<E: GeneralEndpoint>(
        &mut self,
        endpoint: &E,
        now_ns: i64,
    ) -> Result<EntityReconciliationReport, EntityReconciliationError> {
        crate::perception_entity_reconciliation_runtime::reconcile_phy(self, endpoint, now_ns)
    }

    pub(crate) fn prepare_entity_reconciliation(
        &self,
    ) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
        self.prepare_entity_reconciliation_runtime()
    }

    pub(crate) fn commit_entity_reconciliation(
        &mut self,
        prepared: EntityReconciliationPrepared,
        evaluation: EntityReconciliationEvaluation,
        now_ns: i64,
    ) -> Result<
        Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
        EntityReconciliationError,
    > {
        self.commit_entity_reconciliation_runtime(prepared, evaluation, now_ns)
    }
}
