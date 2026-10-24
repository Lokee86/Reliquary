use crate::container::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::{MemoryReadView, MemoryStore};
use crate::relationship_store::RelationshipStore;
use crate::{EntityError, EntityMergeOutcome, GraphError, MemoryError, RelationshipError};

pub(crate) fn apply_entity_merge_plan<E>(
    publication: crate::entity_merge::EntityMergePublicationPlan,
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
    let crate::entity_merge::EntityMergePublicationPlan {
        merge: plan,
        graph_changes: changes,
        associations_retargeted,
        owner_id,
        relationships: writes,
    } = publication;
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

    owner_id.ok_or(RelationshipError::MissingOwnerIdentity)?;
    let mut relationships_retargeted = 0;
    for write in writes {
        let write = write?;
        relationships.publish(
            container,
            Some(write.id),
            write.expected_revision,
            write.draft,
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

/// Ephemeral native publication capability for one reconciliation operation.
/// It borrows the already-open canonical stores; it owns no state or inference.
pub(crate) struct ReconciliationPublication<'a> {
    container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) resolutions: &'a mut EntityResolutionStore,
    pub(crate) graph: &'a mut GraphStore,
    pub(crate) relationships: &'a mut RelationshipStore,
}
impl<'a> ReconciliationPublication<'a> {
    pub(crate) fn new(
        container: &'a mut Container,
        memories: &'a MemoryStore,
        entities: &'a mut EntityStore,
        resolutions: &'a mut EntityResolutionStore,
        graph: &'a mut GraphStore,
        relationships: &'a mut RelationshipStore,
    ) -> Self {
        Self {
            container,
            memories,
            entities,
            resolutions,
            graph,
            relationships,
        }
    }
    pub(crate) fn memory_read(&self) -> MemoryReadView<'_> {
        MemoryReadView::new(self.memories, self.container)
    }
    pub(crate) fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    pub(crate) fn apply_merge<E>(
        &mut self,
        plan: crate::entity_merge::EntityMergePublicationPlan,
    ) -> Result<EntityMergeOutcome, E>
    where
        E: From<EntityError> + From<MemoryError> + From<GraphError> + From<RelationshipError>,
    {
        apply_entity_merge_plan(
            plan,
            self.container,
            self.memories,
            self.entities,
            self.resolutions,
            self.graph,
            self.relationships,
        )
    }
    pub(crate) fn reset_resolution(
        &mut self,
        key: crate::MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<(), MemoryError> {
        let write = super::entity_resolution_admission::ResolutionWriteContext {
            memories: self.memories,
            entities: self.entities,
            entity_resolutions: self.resolutions,
        }
        .reset_entity_resolution(key, now_ns)?;
        super::entity_resolution_admission::apply_resolution_write_plan(
            self.container,
            self.resolutions,
            write,
        )?;
        Ok(())
    }
    pub(crate) fn sync(&mut self) -> Result<(), crate::ContainerError> {
        self.container.sync()
    }
}
