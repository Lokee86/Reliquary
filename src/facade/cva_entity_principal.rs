use crate::entity_principal::{PrincipalContext, PrincipalResolutionPlan};
use crate::memory_store::MemoryReadView;
use crate::{
    Cva, EntityResolutionOutcome, EntityResolverError, MemoryEntityMentionKey, Phylactery,
};

fn apply_principal_plan(
    owner: &mut Cva,
    mut plan: PrincipalResolutionPlan,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if !plan.should_apply {
        return Ok(plan.outcome);
    }
    if let Some((id, draft)) = plan.entity_draft.take() {
        owner
            .entities
            .publish(&mut owner.container, Some(id), 0, draft)?;
    }
    if plan.associate {
        let entity_id = plan
            .outcome
            .entity_id
            .expect("association plan has an Entity");
        plan.outcome.association_changed = owner
            .set_principal_association(
                plan.outcome.key.memory_id,
                entity_id,
                true,
                owner.graph_version(),
            )?
            .is_some();
        plan.outcome.resolution_changed = owner.put_entity_resolved(
            plan.outcome.key,
            plan.expected_revision,
            entity_id,
            plan.outcome.reason,
            now_ns,
        )?;
    } else if plan.outcome.decision == crate::EntityResolutionDecision::Reject {
        plan.outcome.resolution_changed = owner.put_entity_rejected(
            plan.outcome.key,
            plan.expected_revision,
            plan.outcome.reason,
            now_ns,
        )?;
    }
    Ok(plan.outcome)
}

impl Cva {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        let plan = {
            let memories = MemoryReadView::new(&self.memories, &self.container);
            PrincipalContext {
                memories,
                entities: &self.entities,
                entity_resolutions: &self.entity_resolutions,
                graph: &self.graph,
                archive: Some(&self.archive),
            }
            .resolve_principal_entity_mention(key, now_ns)?
        };
        plan.map(|plan| apply_principal_plan(self, plan, now_ns))
            .transpose()
    }

    pub(crate) fn sync_principal_profile(
        &mut self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<bool, crate::EntityError> {
        let plan = PrincipalContext {
            memories: MemoryReadView::new(&self.memories, &self.container),
            entities: &self.entities,
            entity_resolutions: &self.entity_resolutions,
            graph: &self.graph,
            archive: Some(&self.archive),
        }
        .sync_principal_profile(principal_id, profile, now_ns)?;
        let Some((id, revision, draft)) = plan else {
            return Ok(false);
        };
        self.entities
            .publish(&mut self.container, Some(id), revision, draft)
            .map(|(_, changed)| changed)
    }
}

impl Phylactery {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        let plan = PrincipalContext {
            memories: MemoryReadView::new(&self.memories, &self.container),
            entities: &self.entities,
            entity_resolutions: &self.entity_resolutions,
            graph: &self.graph,
            archive: None,
        }
        .reject_phy_principal_entity_mention(key, now_ns)?;
        let Some(mut plan) = plan else {
            return Ok(None);
        };
        if plan.should_apply {
            plan.outcome.resolution_changed = self.put_entity_rejected(
                plan.outcome.key,
                plan.expected_revision,
                plan.outcome.reason,
                now_ns,
            )?;
        }
        Ok(Some(plan.outcome))
    }
}
