use crate::entity_resolution_commit::{
    self, EntityResolutionCommitContext, EntityResolutionCommitOperation,
    EntityResolutionCommitPlan,
};
use crate::{
    Cva, EntityResolutionDecision, EntityResolutionEvaluation, EntityResolutionOutcome,
    EntityResolutionPrepared, EntityResolverError, Phylactery,
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
                let EntityResolutionCommitPlan { key, decision, reason, operation } = plan;
                let (entity_id, entity_created, association_changed, resolution_changed) = match operation {
                    EntityResolutionCommitOperation::Existing { entity_id, alias_update, expected_revision, now_ns } => {
                        if let Some((revision, draft)) = alias_update {
                            self.publish_entity(Some(entity_id), revision, draft)?;
                        }
                        let association_changed = self.set_entity_association(
                            key.memory_id, entity_id, true, self.graph_version(),
                        )?.is_some();
                        let resolution_changed = self.put_entity_resolved(
                            key, expected_revision, entity_id, reason, now_ns,
                        )?;
                        (Some(entity_id), false, association_changed, resolution_changed)
                    }
                    EntityResolutionCommitOperation::Create { draft, expected_revision, now_ns } => {
                        let (entity, entity_created) = self.publish_entity(None, 0, draft)?;
                        let association_changed = self.set_entity_association(
                            key.memory_id, entity.id, true, self.graph_version(),
                        )?.is_some();
                        let resolution_changed = self.put_entity_resolved(
                            key, expected_revision, entity.id, reason, now_ns,
                        )?;
                        (Some(entity.id), entity_created, association_changed, resolution_changed)
                    }
                    EntityResolutionCommitOperation::Unresolved {
                        candidate_entity_ids, candidate_fingerprint, context_fingerprint,
                        expected_revision, now_ns,
                    } => {
                        let changed = self.put_entity_unresolved(
                            key, expected_revision, candidate_entity_ids, reason,
                            candidate_fingerprint, context_fingerprint, now_ns,
                        )?;
                        (None, false, false, changed)
                    }
                    EntityResolutionCommitOperation::Rejected { expected_revision, now_ns } => {
                        let changed = self.put_entity_rejected(key, expected_revision, reason, now_ns)?;
                        (None, false, false, changed)
                    }
                };
                Ok(EntityResolutionOutcome {
                    key, decision, reason, entity_id, entity_created,
                    association_changed, resolution_changed,
                })
            }

            pub(crate) fn commit_entity_resolution(
                &mut self,
                prepared: EntityResolutionPrepared,
                evaluation: EntityResolutionEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
                let Some(plan) = self.plan_entity_resolution_commit(prepared, evaluation, now_ns)? else {
                    return Ok(None);
                };
                self.apply_entity_resolution_commit_plan(plan).map(Some)
            }
        }
    };
}
impl_owner!(Cva);
impl_owner!(Phylactery);
