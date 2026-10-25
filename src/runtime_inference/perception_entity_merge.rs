use crate::{EntityId, EntityMergeOutcome, MemoryEntityMentionKey};

pub(crate) fn publish_entity_rel(
    owner: &mut crate::RelExecutionOwner,
    id: Option<EntityId>,
    expected_revision: u64,
    draft: crate::EntityDraft,
) -> Result<(crate::Entity, bool), crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::publish(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        id,
        expected_revision,
        draft,
        false,
    )
}

pub(crate) fn publish_entity_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    id: Option<EntityId>,
    expected_revision: u64,
    draft: crate::EntityDraft,
) -> Result<(crate::Entity, bool), crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::publish(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        id,
        expected_revision,
        draft,
        true,
    )
}

pub(crate) fn replay_entity_revision_rel(
    owner: &mut crate::RelExecutionOwner,
    id: EntityId,
    expected_revision: u64,
    draft: crate::EntityDraft,
    global_id: Option<crate::EntityGlobalId>,
) -> Result<(crate::Entity, bool), crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::publish_with_global_id(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        Some(id),
        expected_revision,
        draft,
        global_id,
    )
}

pub(crate) fn replay_entity_revision_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
    expected_revision: u64,
    draft: crate::EntityDraft,
    global_id: Option<crate::EntityGlobalId>,
) -> Result<(crate::Entity, bool), crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::publish_with_global_id(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        Some(id),
        expected_revision,
        draft,
        global_id,
    )
}

pub(crate) fn entity_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
) -> Result<crate::Entity, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::entity(&owner.owner.entities, id)
}
pub(crate) fn entity_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
) -> Result<crate::Entity, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::entity(&owner.owner.entities, id)
}

pub(crate) fn tombstone_entity_rel(
    owner: &mut crate::RelExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<crate::owner_lifecycle::TombstoneOutcome, crate::EntityError> {
    let result = crate::owner_lifecycle::entity_publication_access::tombstone_direct(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        id,
        revision,
    )?;
    owner
        .owner
        .exclude_entity_from_graph(id)
        .map_err(|_| crate::EntityError::InvalidField("Entity graph projection"))?;
    Ok(result)
}
pub(crate) fn tombstone_entity_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<crate::owner_lifecycle::TombstoneOutcome, crate::EntityError> {
    let result = crate::owner_lifecycle::entity_publication_access::tombstone_direct(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        id,
        revision,
    )?;
    owner
        .owner
        .exclude_entity_from_graph(id)
        .map_err(|_| crate::EntityError::InvalidField("Entity graph projection"))?;
    Ok(result)
}

pub(crate) fn entity_retention_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
) -> Result<crate::EntityRetentionDescriptor, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::retention_descriptor(
        &owner.owner.entities,
        id,
    )
}
pub(crate) fn entity_retention_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
) -> Result<crate::EntityRetentionDescriptor, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::retention_descriptor(
        &owner.owner.entities,
        id,
    )
}
pub(crate) fn entity_lifecycle_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
) -> Result<crate::owner_lifecycle::LifecycleState, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::lifecycle_state(&owner.owner.entities, id)
}
pub(crate) fn entity_lifecycle_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
) -> Result<crate::owner_lifecycle::LifecycleState, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::lifecycle_state(&owner.owner.entities, id)
}
pub(crate) fn entity_revision_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<crate::Entity, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::entity_revision(
        &owner.owner.entities,
        id,
        revision,
    )
}
pub(crate) fn entity_revision_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<crate::Entity, crate::EntityError> {
    crate::owner_lifecycle::entity_publication_access::entity_revision(
        &owner.owner.entities,
        id,
        revision,
    )
}
pub(crate) fn entity_candidates_rel(
    owner: &crate::RelExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_candidates_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_candidates_normalized_rel(
    owner: &crate::RelExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_normalized_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_candidates_normalized_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_normalized_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_candidates_alias_rel(
    owner: &crate::RelExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_alias_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_candidates_alias_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    surface: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::candidates_for_alias_surface(
        &owner.owner.entities,
        surface,
        limit,
    )
}
pub(crate) fn entity_creation_conflicts_rel(
    owner: &crate::RelExecutionOwner,
    surface: &str,
    kind: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::creation_conflicts(
        &owner.owner.entities,
        surface,
        kind,
        limit,
    )
}
pub(crate) fn entity_creation_conflicts_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    surface: &str,
    kind: &str,
    limit: usize,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::creation_conflicts(
        &owner.owner.entities,
        surface,
        kind,
        limit,
    )
}
pub(crate) fn entity_retired_replacement_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
) -> Option<EntityId> {
    crate::owner_lifecycle::entity_publication_access::retired_replacement(
        &owner.owner.entities,
        id,
    )
}
pub(crate) fn entity_retired_replacement_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
) -> Option<EntityId> {
    crate::owner_lifecycle::entity_publication_access::retired_replacement(
        &owner.owner.entities,
        id,
    )
}
pub(crate) fn entity_version_rel(owner: &crate::RelExecutionOwner) -> u64 {
    crate::owner_lifecycle::entity_publication_access::entity_version(&owner.owner.entities)
}
pub(crate) fn entity_version_phy(owner: &crate::phy_execution_owner::PhyExecutionOwner) -> u64 {
    crate::owner_lifecycle::entity_publication_access::entity_version(&owner.owner.entities)
}
pub(crate) fn entities_rel(owner: &crate::RelExecutionOwner) -> Vec<crate::Entity> {
    owner.runtime_entities()
}
pub(crate) fn entities_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
) -> Vec<crate::Entity> {
    crate::owner_lifecycle::entity_publication_access::current_entities(&owner.owner.entities)
}
pub(crate) fn entity_stats_rel(owner: &crate::RelExecutionOwner) -> crate::EntityStats {
    owner.runtime_entity_stats()
}
pub(crate) fn entity_stats_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
) -> crate::EntityStats {
    crate::owner_lifecycle::entity_publication_access::stats(&owner.owner.entities)
}

pub(crate) fn historical_entity_rel(
    owner: &crate::RelExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<
    crate::owner_lifecycle::HistoricalPayload<crate::Entity, crate::EntityRetainedRevision>,
    crate::EntityError,
> {
    let value = match entity_revision_rel(owner, id, revision) {
        Ok(value) => value,
        Err(crate::EntityError::PayloadUnavailable(reason)) => {
            let structural = entity_retention_rel(owner, id)?
                .revisions
                .into_iter()
                .find(|entry| entry.revision == revision)
                .ok_or(crate::EntityError::MissingEntity)?;
            return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                structural_history: structural,
                reason,
            });
        }
        Err(error) => return Err(error),
    };
    Ok(match entity_lifecycle_rel(owner, id)? {
        crate::owner_lifecycle::LifecycleState::Active => {
            crate::owner_lifecycle::HistoricalPayload::Available(value)
        }
        crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
            crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
        }
    })
}
pub(crate) fn historical_entity_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    id: EntityId,
    revision: u64,
) -> Result<
    crate::owner_lifecycle::HistoricalPayload<crate::Entity, crate::EntityRetainedRevision>,
    crate::EntityError,
> {
    let value = match entity_revision_phy(owner, id, revision) {
        Ok(value) => value,
        Err(crate::EntityError::PayloadUnavailable(reason)) => {
            let structural = entity_retention_phy(owner, id)?
                .revisions
                .into_iter()
                .find(|entry| entry.revision == revision)
                .ok_or(crate::EntityError::MissingEntity)?;
            return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                structural_history: structural,
                reason,
            });
        }
        Err(error) => return Err(error),
    };
    Ok(match entity_lifecycle_phy(owner, id)? {
        crate::owner_lifecycle::LifecycleState::Active => {
            crate::owner_lifecycle::HistoricalPayload::Available(value)
        }
        crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
            crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
        }
    })
}

pub(crate) fn backfill_entity_global_ids_rel(
    owner: &mut crate::RelExecutionOwner,
) -> Result<usize, crate::EntityError> {
    let missing: Vec<_> = entities_rel(owner)
        .into_iter()
        .filter(|entity| entity.global_id.is_none())
        .collect();
    for entity in &missing {
        publish_entity_rel(
            owner,
            Some(entity.id),
            entity.revision,
            entity_global_id_backfill_draft(entity),
        )?;
    }
    Ok(missing.len())
}
pub(crate) fn backfill_entity_global_ids_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
) -> Result<usize, crate::EntityError> {
    let missing: Vec<_> = entities_phy(owner)
        .into_iter()
        .filter(|entity| entity.global_id.is_none())
        .collect();
    for entity in &missing {
        publish_entity_phy(
            owner,
            Some(entity.id),
            entity.revision,
            entity_global_id_backfill_draft(entity),
        )?;
    }
    Ok(missing.len())
}
fn entity_global_id_backfill_draft(entity: &crate::Entity) -> crate::EntityDraft {
    crate::EntityDraft {
        canonical_name: entity.canonical_name.clone(),
        aliases: entity.aliases.clone(),
        kind: entity.kind.clone(),
        summary: entity.summary.clone(),
        mutation_id: format!(
            "entity-global-uuid-backfill:{}:{}",
            entity
                .id
                .0
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            entity.revision + 1
        ),
        created_at_ns: entity.created_at_ns,
        updated_at_ns: entity.updated_at_ns,
    }
}

pub(crate) fn merge_rel(
    owner: &mut crate::RelExecutionOwner,
    survivor_id: EntityId,
    retired_id: EntityId,
    now_ns: i64,
) -> Result<EntityMergeOutcome, crate::CvaError> {
    let plan = crate::entity_merge::plan_entity_merge(
        &owner.owner.entities,
        survivor_id,
        retired_id,
        now_ns,
    )?;
    crate::owner_lifecycle::perception_publication_merge::apply_entity_merge_plan(
        crate::entity_merge::prepare_merge_publication(
            plan,
            owner.owner.owner_id(),
            &owner.owner.memories,
            &owner.owner.entities,
            &owner.owner.graph,
            &owner.owner.relationships,
        ),
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        &mut owner.owner.relationships,
    )
}

pub(crate) fn merge_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    survivor_id: EntityId,
    retired_id: EntityId,
    now_ns: i64,
) -> Result<EntityMergeOutcome, crate::PhylacteryError> {
    let plan = crate::entity_merge::plan_entity_merge(
        &owner.owner.entities,
        survivor_id,
        retired_id,
        now_ns,
    )?;
    crate::owner_lifecycle::perception_publication_merge::apply_entity_merge_plan(
        crate::entity_merge::prepare_merge_publication(
            plan,
            owner.owner.owner_id(),
            &owner.owner.memories,
            &owner.owner.entities,
            &owner.owner.graph,
            &owner.owner.relationships,
        ),
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        &mut owner.owner.relationships,
    )
}

pub(crate) fn retarget_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    from: EntityId,
    to: EntityId,
    now_ns: i64,
) -> Result<bool, crate::CvaError> {
    let Some(plan) = crate::entity_merge::plan_entity_resolution_retarget::<crate::CvaError>(
        &owner.owner.memories,
        &owner.owner.entities,
        key,
        from,
        to,
    )?
    else {
        return Ok(false);
    };
    let resolution_changed =
        crate::owner_lifecycle::entity_publication_access::retarget_resolution(
            &mut owner.owner.container,
            &mut owner.owner.entity_resolutions,
            key,
            from,
            to,
            now_ns,
        )?;
    let mut changed = resolution_changed;
    if let Some(add) = crate::entity_merge::plan_retarget_add_association(&owner.owner.graph, &plan)
    {
        let expected_graph_version = owner.owner.owner_graph_version();
        changed |=
            crate::owner_lifecycle::perception_publication_merge::apply_retarget_association(
                &mut owner.owner.container,
                &owner.owner.memories,
                &owner.owner.entities,
                &mut owner.owner.graph,
                key.memory_id,
                to,
                true,
                expected_graph_version,
            )?
            .is_some();
    }
    if let Some(remove) = crate::entity_merge::plan_retarget_remove_association(
        &owner.owner.graph,
        &owner.owner.entity_resolutions,
        &plan,
    ) {
        let expected_graph_version = owner.owner.owner_graph_version();
        changed |=
            crate::owner_lifecycle::perception_publication_merge::apply_retarget_association(
                &mut owner.owner.container,
                &owner.owner.memories,
                &owner.owner.entities,
                &mut owner.owner.graph,
                key.memory_id,
                from,
                false,
                expected_graph_version,
            )?
            .is_some();
    }
    Ok(changed)
}

pub(crate) fn retarget_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    from: EntityId,
    to: EntityId,
    now_ns: i64,
) -> Result<bool, crate::PhylacteryError> {
    let Some(plan) = crate::entity_merge::plan_entity_resolution_retarget::<crate::PhylacteryError>(
        &owner.owner.memories,
        &owner.owner.entities,
        key,
        from,
        to,
    )?
    else {
        return Ok(false);
    };
    let resolution_changed =
        crate::owner_lifecycle::entity_publication_access::retarget_resolution(
            &mut owner.owner.container,
            &mut owner.owner.entity_resolutions,
            key,
            from,
            to,
            now_ns,
        )?;
    let mut changed = resolution_changed;
    if let Some(add) = crate::entity_merge::plan_retarget_add_association(&owner.owner.graph, &plan)
    {
        let expected_graph_version = owner.owner.owner_graph_version();
        changed |=
            crate::owner_lifecycle::perception_publication_merge::apply_retarget_association(
                &mut owner.owner.container,
                &owner.owner.memories,
                &owner.owner.entities,
                &mut owner.owner.graph,
                key.memory_id,
                to,
                true,
                expected_graph_version,
            )?
            .is_some();
    }
    if let Some(remove) = crate::entity_merge::plan_retarget_remove_association(
        &owner.owner.graph,
        &owner.owner.entity_resolutions,
        &plan,
    ) {
        let expected_graph_version = owner.owner.owner_graph_version();
        changed |=
            crate::owner_lifecycle::perception_publication_merge::apply_retarget_association(
                &mut owner.owner.container,
                &owner.owner.memories,
                &owner.owner.entities,
                &mut owner.owner.graph,
                key.memory_id,
                from,
                false,
                expected_graph_version,
            )?
            .is_some();
    }
    Ok(changed)
}
