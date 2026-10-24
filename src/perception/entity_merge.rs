use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Entity, EntityDraft, EntityError, EntityId, EntityMergeOutcome, EntityRef, GraphError,
    GraphRelationOrigin, MAX_ENTITY_ALIASES, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryError, Relationship, RelationshipDraft, RelationshipError, SemanticGraphRelationChange,
};

pub(crate) enum EntityMergePlan {
    AlreadyMerged(EntityMergeOutcome),
    Apply {
        survivor: Entity,
        retired: Entity,
        aliases_added: usize,
        alias_draft: Option<EntityDraft>,
        now_ns: i64,
    },
}

/// Concrete mutation facts prepared above native publication. Validation
/// failures are carried per relationship so the owner can surface each one
/// at the corresponding ordered write boundary.
pub(crate) struct EntityMergePublicationPlan {
    pub(crate) merge: EntityMergePlan,
    pub(crate) graph_changes: Vec<SemanticGraphRelationChange>,
    pub(crate) associations_retargeted: usize,
    pub(crate) owner_id: Option<String>,
    pub(crate) relationships: Vec<Result<EntityMergeRelationshipWrite, RelationshipError>>,
}

pub(crate) struct EntityMergeRelationshipWrite {
    pub(crate) id: crate::RelationshipId,
    pub(crate) expected_revision: u64,
    pub(crate) draft: RelationshipDraft,
}

pub(crate) fn plan_entity_merge(
    entities: &EntityStore,
    survivor_id: EntityId,
    retired_id: EntityId,
    now_ns: i64,
) -> Result<EntityMergePlan, EntityError> {
    if survivor_id == retired_id {
        return Err(EntityError::InvalidField("Entity merge identity"));
    }
    if entities.retired_replacement(retired_id) == Some(survivor_id) {
        return Ok(EntityMergePlan::AlreadyMerged(EntityMergeOutcome {
            survivor_id,
            retired_id,
            aliases_added: 0,
            associations_retargeted: 0,
            resolutions_retargeted: 0,
            relationships_retargeted: 0,
            changed: false,
        }));
    }

    let survivor = entities.entity(survivor_id)?;
    let retired = entities.entity(retired_id)?;
    if survivor.kind == "principal" || retired.kind == "principal" {
        return Err(EntityError::InvalidField("principal Entity merge"));
    }
    if survivor.kind != retired.kind {
        return Err(EntityError::InvalidField("Entity merge kind"));
    }

    let active_entities = entities.current();
    let aliases = merged_aliases(&survivor, &retired, &active_entities)?;
    let aliases_added = aliases.len().saturating_sub(survivor.aliases.len());
    let alias_draft = (aliases_added > 0).then(|| EntityDraft {
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
    });
    Ok(EntityMergePlan::Apply {
        survivor,
        retired,
        aliases_added,
        alias_draft,
        now_ns,
    })
}

pub(crate) fn plan_merge_association_changes(
    graph: &GraphStore,
    survivor_id: EntityId,
    retired_id: EntityId,
) -> (usize, Vec<SemanticGraphRelationChange>) {
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
    (affected_memories.len(), changes)
}

pub(crate) fn plan_relationship_retarget(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    survivor_id: EntityId,
    retired_id: EntityId,
    relationship: &Relationship,
    now_ns: i64,
) -> Result<RelationshipDraft, RelationshipError> {
    let retired_ref = EntityRef {
        owner_id: owner_id.to_owned(),
        entity_id: retired_id,
    };
    let mut participants = relationship.participants.clone();
    for participant in &mut participants {
        if participant.entity == retired_ref {
            participant.entity.entity_id = survivor_id;
        }
    }
    let mut draft = RelationshipDraft {
        kind: relationship.kind.clone(),
        participants,
        evidence: relationship.evidence.clone(),
        summary: relationship.summary.clone(),
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
        owner_id, memories, entities, &mut draft,
    )?;
    Ok(draft)
}

pub(crate) struct EntityResolutionRetargetPlan {
    pub(crate) key: MemoryEntityMentionKey,
    pub(crate) from: EntityId,
    pub(crate) to: EntityId,
}

pub(crate) fn plan_entity_resolution_retarget<E>(
    memories: &MemoryStore,
    entities: &EntityStore,
    key: MemoryEntityMentionKey,
    from: EntityId,
    to: EntityId,
) -> Result<Option<EntityResolutionRetargetPlan>, E>
where
    E: From<EntityError> + From<MemoryError>,
{
    if from == to {
        return Ok(None);
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
    Ok(Some(EntityResolutionRetargetPlan { key, from, to }))
}

pub(crate) fn plan_retarget_add_association(
    graph: &GraphStore,
    plan: &EntityResolutionRetargetPlan,
) -> Option<SemanticGraphRelationChange> {
    (!graph
        .entity_associations_for_memory(plan.key.memory_id)
        .contains(&plan.to))
    .then(|| SemanticGraphRelationChange::entity_association(plan.key.memory_id, plan.to, true))
}

pub(crate) fn plan_retarget_remove_association(
    graph: &GraphStore,
    resolutions: &EntityResolutionStore,
    plan: &EntityResolutionRetargetPlan,
) -> Option<SemanticGraphRelationChange> {
    let still_targets_from = resolutions
        .for_memory(plan.key.memory_id)
        .iter()
        .any(|value| {
            matches!(value.status,
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == plan.from)
        });
    (!still_targets_from
        && graph
            .entity_associations_for_memory(plan.key.memory_id)
            .contains(&plan.from))
    .then(|| SemanticGraphRelationChange::entity_association(plan.key.memory_id, plan.from, false))
}

pub(crate) fn validate_retarget_association(
    memories: &MemoryStore,
    entities: &EntityStore,
    change: SemanticGraphRelationChange,
) -> Result<(), GraphError> {
    crate::graph_endpoint_admission::validate_semantic_changes(
        memories,
        entities,
        &[change],
        GraphRelationOrigin::Perception,
    )
}

pub(crate) fn merged_aliases(
    survivor: &Entity,
    retired: &Entity,
    active_entities: &[Entity],
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
    active_entities: &[Entity],
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

pub(crate) fn prepare_merge_publication(
    merge: EntityMergePlan,
    owner_id: Option<String>,
    memories: &MemoryStore,
    entities: &EntityStore,
    graph: &GraphStore,
    relationships: &RelationshipStore,
) -> EntityMergePublicationPlan {
    let (associations_retargeted, graph_changes, writes) = match &merge {
        EntityMergePlan::AlreadyMerged(_) => (0, Vec::new(), Vec::new()),
        EntityMergePlan::Apply {
            survivor,
            retired,
            now_ns,
            ..
        } => {
            let (count, changes) = plan_merge_association_changes(graph, survivor.id, retired.id);
            let writes = owner_id
                .as_ref()
                .map(|owner| {
                    relationships
                        .for_entity(&EntityRef {
                            owner_id: owner.clone(),
                            entity_id: retired.id,
                        })
                        .into_iter()
                        .map(|relationship| {
                            plan_relationship_retarget(
                                owner,
                                memories,
                                entities,
                                survivor.id,
                                retired.id,
                                &relationship,
                                *now_ns,
                            )
                            .map(|draft| {
                                EntityMergeRelationshipWrite {
                                    id: relationship.id,
                                    expected_revision: relationship.revision,
                                    draft,
                                }
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            (count, changes, writes)
        }
    };
    EntityMergePublicationPlan {
        merge,
        graph_changes,
        associations_retargeted,
        owner_id,
        relationships: writes,
    }
}
