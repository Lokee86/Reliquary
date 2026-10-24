use crate::container::Container;
use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::entity_resolution_owner::{ResolutionWriteContext, ResolutionWritePlan};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::{MemoryReadView, MemoryStore};
use crate::relationship_store::RelationshipStore;
use crate::{
    EntityError, EntityMergeOutcome, EntityReconciliationError, GraphError, MemoryError,
    RelationshipError,
};

use super::perception_publication_resolution::{
    ResolutionWriteResult, apply_resolution_write_plan,
};

pub(crate) fn prepare_entity_reconciliation(
    container: &Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    graph: &GraphStore,
    resolutions: &EntityResolutionStore,
) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
    let memory_view = MemoryReadView::new(memories, container);
    let context = crate::entity_reconciliation_prepare::ReconciliationPrepareContext {
        entities,
        graph,
        memories: &memory_view,
        resolutions,
    };
    crate::entity_reconciliation_prepare::prepare_entity_reconciliation(&context)
}

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

pub(crate) fn commit_entity_reconciliation<E>(
    container: &mut Container,
    memories: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    relationships: &mut RelationshipStore,
    prepared: EntityReconciliationPrepared,
    evaluation: EntityReconciliationEvaluation,
    now_ns: i64,
) -> Result<
    Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
    EntityReconciliationError,
>
where
    E: From<EntityError>
        + From<MemoryError>
        + From<GraphError>
        + From<RelationshipError>
        + std::fmt::Display,
{
    let memory_view = MemoryReadView::new(memories, container);
    let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
        memories: &memory_view,
        entities,
        graph,
        resolutions,
    };
    let Some(plan) = crate::entity_reconciliation_commit::plan_entity_reconciliation_commit(
        &context, prepared, evaluation,
    ) else {
        return Ok(None);
    };

    let mut timestamp = now_ns;
    let mut deterministic_merges = 0usize;
    let mut model_merges = 0usize;
    for decision in &plan.decisions {
        let planned = {
            let memory_view = MemoryReadView::new(memories, container);
            let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
                memories: &memory_view,
                entities,
                graph,
                resolutions,
            };
            crate::entity_reconciliation_commit::plan_reconciliation_merge(
                &context, &plan, decision, timestamp,
            )?
        };
        let Some((merge_plan, deterministic)) = planned else {
            continue;
        };
        timestamp = timestamp.saturating_add(1);
        let outcome = apply_entity_merge_plan::<E>(
            merge_plan,
            container,
            memories,
            entities,
            resolutions,
            graph,
            relationships,
        )
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if outcome.changed {
            if deterministic {
                deterministic_merges += 1;
            } else {
                model_merges += 1;
            }
        }
    }

    let mut wake_keys = Vec::new();
    for key in plan.late_rejected.iter().copied() {
        let wake = {
            let memory_view = MemoryReadView::new(memories, container);
            let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
                memories: &memory_view,
                entities,
                graph,
                resolutions,
            };
            crate::entity_reconciliation_commit::plan_late_rejection_wake(&context, key)
        };
        let Some(wake) = wake else {
            continue;
        };
        timestamp = timestamp.saturating_add(1);
        let write = ResolutionWriteContext {
            memories,
            entities,
            entity_resolutions: resolutions,
        }
        .reset_entity_resolution(wake.key, timestamp)
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        match apply_resolution_write_plan(container, resolutions, write)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?
        {
            ResolutionWriteResult::Changed(_) => {}
            _ => unreachable!(),
        }
        wake_keys.push(wake.key);
    }

    container
        .sync()
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    let report = crate::EntityReconciliationReport {
        rounds: 1,
        candidate_pairs: plan.candidate_pairs,
        deterministic_merges,
        model_merges,
        rejected_mentions_reconsidered: wake_keys.len(),
        unresolved_pairs: plan
            .candidate_pairs
            .saturating_sub(deterministic_merges + model_merges),
        final_audit: crate::entity_audit::audit_owner(&crate::entity_audit::EntityAuditContext {
            entities,
            memories,
            graph,
            resolutions,
        }),
    };
    Ok(Some(
        crate::entity_reconciliation_model::EntityReconciliationCommit { report, wake_keys },
    ))
}
