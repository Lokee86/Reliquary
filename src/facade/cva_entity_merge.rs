use crate::container::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Cva, CvaError, EntityError, EntityId, EntityMergeOutcome, GraphError, MemoryEntityMentionKey,
    MemoryError, Phylactery, PhylacteryError, RelationshipError,
};

pub(crate) fn apply_entity_merge_plan<E>(
    plan: crate::entity_merge::EntityMergePlan,
    container: &mut Container,
    memories: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    relationships: &mut RelationshipStore,
) -> Result<EntityMergeOutcome, E>
where
    E: From<EntityError> + From<MemoryError> + From<GraphError> + From<RelationshipError>,
{
    let (survivor, retired, aliases_added, alias_draft, now_ns) = match plan {
        crate::entity_merge::EntityMergePlan::AlreadyMerged(outcome) => return Ok(outcome),
        crate::entity_merge::EntityMergePlan::Apply {
            survivor,
            retired,
            aliases_added,
            alias_draft,
            now_ns,
        } => (survivor, retired, aliases_added, alias_draft, now_ns),
    };

    if let Some(draft) = alias_draft {
        entities.publish(container, Some(survivor.id), survivor.revision, draft)?;
    }

    if !entities.contains(survivor.id) {
        return Err(MemoryError::InvalidField("Entity resolution Entity reference").into());
    }
    let resolutions_retargeted =
        resolutions.retarget_entity_references(container, retired.id, survivor.id, now_ns)?;

    let (associations_retargeted, changes) =
        crate::entity_merge::plan_merge_association_changes(graph, survivor.id, retired.id);
    if !changes.is_empty() {
        crate::graph_endpoint_admission::validate_semantic_changes(
            memories,
            entities,
            &changes,
            crate::GraphRelationOrigin::Perception,
        )?;
        graph.set_semantic_relations_with_origin(
            container,
            &changes,
            crate::GraphRelationOrigin::Perception,
            graph.graph_version(),
        )?;
    }

    let owner_id = container
        .owner_id()
        .ok_or(RelationshipError::MissingOwnerIdentity)?;
    let retired_ref = crate::EntityRef {
        owner_id: owner_id.clone(),
        entity_id: retired.id,
    };
    let affected_relationships = relationships.for_entity(&retired_ref);
    let mut relationships_retargeted = 0;
    for relationship in affected_relationships {
        let draft = crate::entity_merge::plan_relationship_retarget(
            &owner_id,
            memories,
            entities,
            survivor.id,
            retired.id,
            &relationship,
            now_ns,
        )?;
        relationships.publish(
            container,
            Some(relationship.id),
            relationship.revision,
            draft,
        )?;
        relationships_retargeted += 1;
    }

    let current_retired = entities.entity(retired.id)?;
    entities.tombstone(container, retired.id, current_retired.revision, survivor.id)?;
    Ok(EntityMergeOutcome {
        survivor_id: survivor.id,
        retired_id: retired.id,
        aliases_added,
        associations_retargeted,
        resolutions_retargeted,
        relationships_retargeted,
        changed: true,
    })
}

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
                apply_entity_merge_plan(
                    plan,
                    &mut self.container,
                    &self.memories,
                    &mut self.entities,
                    &mut self.entity_resolutions,
                    &mut self.graph,
                    &mut self.relationships,
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

                let resolution_changed = self.entity_resolutions.retarget_resolved(
                    &mut self.container,
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
                        &self.entities,
                        add,
                    )?;
                    changed |= self
                        .graph
                        .set_entity_association(
                            &mut self.container,
                            key.memory_id,
                            to,
                            true,
                            self.graph.graph_version(),
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
                        &self.entities,
                        remove,
                    )?;
                    changed |= self
                        .graph
                        .set_entity_association(
                            &mut self.container,
                            key.memory_id,
                            from,
                            false,
                            self.graph.graph_version(),
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
