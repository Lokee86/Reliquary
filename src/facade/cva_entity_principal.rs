use crate::owner_lifecycle::perception_publication_resolution::{
    apply_principal_profile_sync_plan, apply_principal_resolution_plan,
};
use crate::perception_publication_resolution::{
    plan_phy_principal_resolution, plan_principal_profile_sync, plan_rel_principal_resolution,
};
use crate::{
    Cva, EntityResolutionOutcome, EntityResolverError, MemoryEntityMentionKey, Phylactery,
};

impl Cva {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        let memory_view = crate::memory_store::MemoryReadView::new(&self.memories, &self.container);
        let plan = plan_rel_principal_resolution(
            memory_view,
            &self.entities,
            &self.entity_resolutions,
            &self.graph,
            &self.archive,
            key,
            now_ns,
        )?;
        plan.map(|plan| {
            apply_principal_resolution_plan(
                &mut self.container,
                &self.memories,
                &mut self.entities,
                &mut self.entity_resolutions,
                &mut self.graph,
                plan,
                now_ns,
            )
        })
        .transpose()
    }

    pub(crate) fn sync_principal_profile(
        &mut self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<bool, crate::EntityError> {
        let memory_view = crate::memory_store::MemoryReadView::new(&self.memories, &self.container);
        let plan = plan_principal_profile_sync(
            memory_view,
            &self.entities,
            &self.entity_resolutions,
            &self.graph,
            &self.archive,
            principal_id,
            profile,
            now_ns,
        )?;
        apply_principal_profile_sync_plan(
            &mut self.container,
            &mut self.entities,
            plan,
        )
    }
}

impl Phylactery {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        let memory_view = crate::memory_store::MemoryReadView::new(&self.memories, &self.container);
        let plan = plan_phy_principal_resolution(
            memory_view,
            &self.entities,
            &self.entity_resolutions,
            &self.graph,
            key,
            now_ns,
        )?;
        let Some(plan) = plan else {
            return Ok(None);
        };
        apply_principal_resolution_plan(
            &mut self.container,
            &self.memories,
            &mut self.entities,
            &mut self.entity_resolutions,
            &mut self.graph,
            plan,
            now_ns,
        )
        .map(Some)
    }
}
