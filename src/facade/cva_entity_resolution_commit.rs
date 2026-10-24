use crate::entity_resolution_commit::EntityResolutionCommitPlan;
use crate::{
    Cva, EntityResolutionEvaluation, EntityResolutionOutcome, EntityResolutionPrepared,
    EntityResolverError, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn plan_entity_resolution_commit(
                &self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionCommitPlan>, EntityResolverError> {
                crate::perception_entity_resolution_runtime::plan_commit_rel(
                    self, prepared, evaluation, now_ns,
                )
            }

            pub(crate) fn apply_entity_resolution_commit_plan(
                &mut self,
                plan: EntityResolutionCommitPlan,
            ) -> Result<EntityResolutionOutcome, EntityResolverError> {
                crate::perception_entity_resolution_runtime::apply_commit_rel(self, plan)
            }

            pub(crate) fn commit_entity_resolution(
                &mut self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
                let Some(plan) =
                    self.plan_entity_resolution_commit(prepared, evaluation, now_ns)?
                else {
                    return Ok(None);
                };
                self.apply_entity_resolution_commit_plan(plan).map(Some)
            }
        }
    };
}
impl_owner!(Cva);

impl Phylactery {
    pub(crate) fn plan_entity_resolution_commit(
        &self,
        prepared: EntityResolutionPrepared,
        evaluation: EntityResolutionEvaluation,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionCommitPlan>, EntityResolverError> {
        crate::perception_entity_resolution_runtime::plan_commit_phy(
            self, prepared, evaluation, now_ns,
        )
    }

    pub(crate) fn apply_entity_resolution_commit_plan(
        &mut self,
        plan: EntityResolutionCommitPlan,
    ) -> Result<EntityResolutionOutcome, EntityResolverError> {
        crate::perception_entity_resolution_runtime::apply_commit_phy(self, plan)
    }

    pub(crate) fn commit_entity_resolution(
        &mut self,
        prepared: EntityResolutionPrepared,
        evaluation: EntityResolutionEvaluation,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        crate::perception_entity_resolution_runtime::commit_phy(self, prepared, evaluation, now_ns)
    }
}
