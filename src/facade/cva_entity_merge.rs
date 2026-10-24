use crate::{
    Cva, CvaError, EntityId, EntityMergeOutcome, MemoryEntityMentionKey, Phylactery,
    PhylacteryError,
};

macro_rules! impl_entity_merge {
    ($owner:ty, $error:ty) => {
        impl $owner {
            pub fn merge_entities(
                &mut self,
                survivor_id: EntityId,
                retired_id: EntityId,
                now_ns: i64,
            ) -> Result<EntityMergeOutcome, $error> {
                let plan = crate::entity_merge::plan_entity_merge(
                    &self.entities,
                    survivor_id,
                    retired_id,
                    now_ns,
                )?;
                crate::owner_lifecycle::perception_publication_merge::apply_entity_merge_plan(
                    crate::entity_merge::prepare_merge_publication(
                        plan,
                        self.container.owner_id(),
                        &self.memories,
                        &self.entities,
                        &self.graph,
                        &self.relationships,
                    ),
                    &mut self.owner.container,
                    &self.owner.memories,
                    &mut self.owner.entities,
                    &mut self.owner.entity_resolutions,
                    &mut self.owner.graph,
                    &mut self.owner.relationships,
                )
            }

            pub fn retarget_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                from: EntityId,
                to: EntityId,
                now_ns: i64,
            ) -> Result<bool, $error> {
                let Some(plan) = crate::entity_merge::plan_entity_resolution_retarget::<$error>(
                    &self.memories,
                    &self.entities,
                    key,
                    from,
                    to,
                )?
                else {
                    return Ok(false);
                };

                let resolution_changed = self.owner.entity_resolutions.retarget_resolved(
                    &mut self.owner.container,
                    key,
                    from,
                    to,
                    now_ns,
                )?;
                let mut changed = resolution_changed;
                if let Some(add) =
                    crate::entity_merge::plan_retarget_add_association(&self.graph, &plan)
                {
                    crate::entity_merge::validate_retarget_association(
                        &self.memories,
                        &self.owner.entities,
                        add,
                    )?;
                    changed |= self
                        .owner
                        .graph
                        .set_entity_association(
                            &mut self.owner.container,
                            key.memory_id,
                            to,
                            true,
                            self.owner.graph.graph_version(),
                        )?
                        .is_some();
                }
                if let Some(remove) = crate::entity_merge::plan_retarget_remove_association(
                    &self.graph,
                    &self.entity_resolutions,
                    &plan,
                ) {
                    crate::entity_merge::validate_retarget_association(
                        &self.memories,
                        &self.owner.entities,
                        remove,
                    )?;
                    changed |= self
                        .owner
                        .graph
                        .set_entity_association(
                            &mut self.owner.container,
                            key.memory_id,
                            from,
                            false,
                            self.owner.graph.graph_version(),
                        )?
                        .is_some();
                }
                Ok(changed)
            }
        }
    };
}

impl_entity_merge!(Cva, CvaError);
impl_entity_merge!(Phylactery, PhylacteryError);
