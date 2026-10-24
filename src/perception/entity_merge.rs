use crate::container::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Entity, EntityDraft, EntityError, EntityId, EntityMergeOutcome, EntityRef, GraphError,
    GraphRelationOrigin, MAX_ENTITY_ALIASES, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryError, RelationshipDraft, RelationshipError, SemanticGraphRelationChange,
};

pub(crate) fn merge_entities_parts<E>(
    container: &mut Container,
    memories_store: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    relationships: &mut RelationshipStore,
    survivor_id: EntityId,
    retired_id: EntityId,
    now_ns: i64,
) -> Result<EntityMergeOutcome, E>
where
    E: From<EntityError> + From<MemoryError> + From<GraphError> + From<RelationshipError>,
{
    if survivor_id == retired_id {
        return Err(EntityError::InvalidField("Entity merge identity").into());
    }
    if entities.retired_replacement(retired_id) == Some(survivor_id) {
        return Ok(EntityMergeOutcome {
            survivor_id,
            retired_id,
            aliases_added: 0,
            associations_retargeted: 0,
            resolutions_retargeted: 0,
            relationships_retargeted: 0,
            changed: false,
        });
    }

    let survivor = entities.entity(survivor_id)?;
    let retired = entities.entity(retired_id)?;
    if survivor.kind == "principal" || retired.kind == "principal" {
        return Err(EntityError::InvalidField("principal Entity merge").into());
    }
    if survivor.kind != retired.kind {
        return Err(EntityError::InvalidField("Entity merge kind").into());
    }

    let active_entities = entities.current();
    let aliases = merged_aliases(&survivor, &retired, &active_entities)?;
    let aliases_added = aliases.len().saturating_sub(survivor.aliases.len());
    if aliases_added > 0 {
        entities.publish(
            container,
            Some(survivor_id),
            survivor.revision,
            EntityDraft {
                canonical_name: survivor.canonical_name.clone(),
                aliases,
                kind: survivor.kind.clone(),
                summary: survivor.summary.clone(),
                mutation_id: format!(
                    "entity-merge:{}:{}:{}",
                    short_id(survivor_id),
                    short_id(retired_id),
                    survivor.revision + 1
                ),
                created_at_ns: survivor.created_at_ns,
                updated_at_ns: now_ns.max(survivor.updated_at_ns),
            },
        )?;
    }

    if !entities.contains(survivor_id) {
        return Err(MemoryError::InvalidField("Entity resolution Entity reference").into());
    }
    let resolutions_retargeted =
        resolutions.retarget_entity_references(container, retired_id, survivor_id, now_ns)?;

    let affected_memories = graph.memories_for_entity(retired_id);
    let mut changes = Vec::with_capacity(affected_memories.len().saturating_mul(2));
    for memory_id in &affected_memories {
        if !graph
            .entity_associations_for_memory(*memory_id)
            .contains(&survivor_id)
        {
            changes.push(SemanticGraphRelationChange::entity_association(
                *memory_id,
                survivor_id,
                true,
            ));
        }
        changes.push(SemanticGraphRelationChange::entity_association(
            *memory_id, retired_id, false,
        ));
    }
    if !changes.is_empty() {
        crate::graph_endpoint_admission::validate_semantic_changes(
            memories_store,
            entities,
            &changes,
            GraphRelationOrigin::Perception,
        )?;
        graph.set_semantic_relations_with_origin(
            container,
            &changes,
            GraphRelationOrigin::Perception,
            graph.graph_version(),
        )?;
    }

    let owner_id = container
        .owner_id()
        .ok_or(RelationshipError::MissingOwnerIdentity)?;
    let retired_ref = EntityRef {
        owner_id: owner_id.clone(),
        entity_id: retired_id,
    };
    let affected_relationships = relationships.for_entity(&retired_ref);
    let mut relationships_retargeted = 0;
    for relationship in affected_relationships {
        let mut participants = relationship.participants.clone();
        for participant in &mut participants {
            if participant.entity == retired_ref {
                participant.entity.entity_id = survivor_id;
            }
        }
        let mut draft = RelationshipDraft {
            kind: relationship.kind,
            participants,
            evidence: relationship.evidence,
            summary: relationship.summary,
            mutation_id: format!(
                "entity-merge-relationship:{}:{}:{}:{}",
                short_id(survivor_id),
                short_id(retired_id),
                short_bytes(&relationship.id.0),
                relationship.revision + 1
            ),
            created_at_ns: relationship.created_at_ns,
            updated_at_ns: now_ns.max(relationship.updated_at_ns),
        };
        crate::relationship_owner::validate_live_relationship_refs(
            &owner_id,
            memories_store,
            entities,
            &mut draft,
        )?;
        relationships.publish(
            container,
            Some(relationship.id),
            relationship.revision,
            draft,
        )?;
        relationships_retargeted += 1;
    }

    let retired = entities.entity(retired_id)?;
    entities.tombstone(container, retired_id, retired.revision, survivor_id)?;
    Ok(EntityMergeOutcome {
        survivor_id,
        retired_id,
        aliases_added,
        associations_retargeted: affected_memories.len(),
        resolutions_retargeted,
        relationships_retargeted,
        changed: true,
    })
}

pub(crate) fn retarget_entity_resolution_parts<E>(
    container: &mut Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    key: MemoryEntityMentionKey,
    from: EntityId,
    to: EntityId,
    now_ns: i64,
) -> Result<bool, E>
where
    E: From<EntityError> + From<MemoryError> + From<GraphError>,
{
    if from == to {
        return Ok(false);
    }
    let from_entity = entities.entity(from)?;
    let to_entity = entities.entity(to)?;
    if from_entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
        || to_entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
    {
        return Err(EntityError::InvalidField("principal Entity retarget").into());
    }
    memories.validate_entity_mention_key(key)?;
    if !entities.contains(to) {
        return Err(MemoryError::InvalidField("Entity resolution Entity reference").into());
    }
    let resolution_changed = resolutions.retarget_resolved(container, key, from, to, now_ns)?;
    let memory_id = key.memory_id;
    let mut changed = resolution_changed;
    if !graph
        .entity_associations_for_memory(memory_id)
        .contains(&to)
    {
        let change = SemanticGraphRelationChange::entity_association(memory_id, to, true);
        crate::graph_endpoint_admission::validate_semantic_changes(
            memories,
            entities,
            &[change],
            GraphRelationOrigin::Perception,
        )?;
        changed |= graph
            .set_entity_association(container, memory_id, to, true, graph.graph_version())?
            .is_some();
    }
    let still_targets_from = resolutions.for_memory(memory_id).iter().any(|value| {
        matches!(value.status,
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == from)
    });
    if !still_targets_from
        && graph
            .entity_associations_for_memory(memory_id)
            .contains(&from)
    {
        let change = SemanticGraphRelationChange::entity_association(memory_id, from, false);
        crate::graph_endpoint_admission::validate_semantic_changes(
            memories,
            entities,
            &[change],
            GraphRelationOrigin::Perception,
        )?;
        changed |= graph
            .set_entity_association(container, memory_id, from, false, graph.graph_version())?
            .is_some();
    }
    Ok(changed)
}

pub(crate) fn merged_aliases(
    survivor: &crate::Entity,
    retired: &crate::Entity,
    active_entities: &[crate::Entity],
) -> Result<Vec<String>, EntityError> {
    let mut aliases = survivor.aliases.clone();
    for value in std::iter::once(retired.canonical_name.as_str())
        .chain(retired.aliases.iter().map(String::as_str))
    {
        if survivor.canonical_name.eq_ignore_ascii_case(value)
            || aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value))
            || surface_claimed_by_other_entity(value, survivor.id, retired.id, active_entities)
        {
            continue;
        }
        aliases.push(value.to_owned());
    }
    aliases.sort();
    aliases.dedup();
    if aliases.len() > MAX_ENTITY_ALIASES {
        return Err(EntityError::FieldTooLarge);
    }
    Ok(aliases)
}

fn surface_claimed_by_other_entity(
    surface: &str,
    survivor_id: EntityId,
    retired_id: EntityId,
    active_entities: &[crate::Entity],
) -> bool {
    active_entities.iter().any(|entity| {
        entity.id != survivor_id
            && entity.id != retired_id
            && (entity.canonical_name.eq_ignore_ascii_case(surface)
                || entity
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(surface)))
    })
}

pub(crate) fn short_id(id: EntityId) -> String {
    short_bytes(&id.0)
}

pub(crate) fn short_bytes(bytes: &[u8; 32]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
