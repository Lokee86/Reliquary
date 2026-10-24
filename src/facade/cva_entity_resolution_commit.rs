use crate::entity_resolution_commit::{
    self, EntityResolutionCommitContext, EntityResolutionCommitPlan,
};
use crate::owner_lifecycle::perception_publication_resolution::
    apply_entity_resolution_commit_plan;
use crate::{
    Cva, EntityResolutionEvaluation, EntityResolutionOutcome, EntityResolutionPrepared,
    EntityResolverError, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty, $reject_principal:expr) => {
        impl $owner {
            pub(crate) fn plan_entity_resolution_commit(
                &self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionCommitPlan>, EntityResolverError> {
                let context = EntityResolutionCommitContext {
                    owner_id: self.owner_id(),
                    memories: &self.memories,
                    entities: &self.entities,
                    graph: &self.graph,
                    resolutions: &self.entity_resolutions,
                };
                entity_resolution_commit::plan_entity_resolution_commit(
                    &context, prepared, evaluation, now_ns,
                )
            }

            pub(crate) fn apply_entity_resolution_commit_plan(
                &mut self,
                plan: EntityResolutionCommitPlan,
            ) -> Result<EntityResolutionOutcome, EntityResolverError> {
                apply_entity_resolution_commit_plan(
                    &mut self.container,
                    &self.memories,
                    &mut self.entities,
                    &mut self.entity_resolutions,
                    &mut self.graph,
                    plan,
                    $reject_principal,
                )
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
impl_owner!(Cva, false);
impl_owner!(Phylactery, true);
