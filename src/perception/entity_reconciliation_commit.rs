use crate::entity_reconciliation_model::{
    EntityReconciliationCommit, EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::entity_reconciliation_support::{
    choose_survivor, exact_entity_for_surface, merge_alias_budget_fits,
};
use crate::entity_resolution_owner::ResolutionWriteContext;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Container, EntityId, EntityReconciliationError, EntityReconciliationRelation,
    EntityReconciliationReport, MemoryEntityResolutionStatus,
};
use std::collections::HashMap;

pub(crate) struct ReconciliationCommitContext<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) graph: &'a mut GraphStore,
    pub(crate) resolutions: &'a mut EntityResolutionStore,
    pub(crate) relationships: &'a mut RelationshipStore,
}

pub(crate) fn commit_entity_reconciliation(
    context: &mut ReconciliationCommitContext<'_>,
    prepared: EntityReconciliationPrepared,
    evaluation: EntityReconciliationEvaluation,
    now_ns: i64,
) -> Result<Option<EntityReconciliationCommit>, EntityReconciliationError> {
    if context.entities.entity_version() != prepared.entity_version
        || context.graph.graph_version() != prepared.graph_version
        || context.memories.memory_version() != prepared.memory_version
    {
        return Ok(None);
    }
    let mut timestamp = now_ns;
    let mut deterministic_merges = 0usize;
    let mut model_merges = 0usize;
    let degrees = context
        .entities
        .current()
        .into_iter()
        .map(|entity| {
            (
                entity.id,
                context.graph.memories_for_entity(entity.id).len(),
            )
        })
        .collect::<HashMap<EntityId, usize>>();

    for decision in evaluation.decisions {
        if decision.relation != EntityReconciliationRelation::SameIdentity
            || context
                .entities
                .retired_replacement(decision.left)
                .is_some()
            || context
                .entities
                .retired_replacement(decision.right)
                .is_some()
        {
            continue;
        }
        let left = context
            .entities
            .entity(decision.left)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        let right = context
            .entities
            .entity(decision.right)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if left.kind != right.kind {
            continue;
        }
        let (survivor, retired) = choose_survivor(&left, &right, &degrees);
        let (survivor_entity, retired_entity) = if survivor == left.id {
            (&left, &right)
        } else {
            (&right, &left)
        };
        if !merge_alias_budget_fits(survivor_entity, retired_entity) {
            continue;
        }
        timestamp = timestamp.saturating_add(1);
        let outcome = crate::entity_merge::merge_entities_parts::<crate::CvaError>(
            context.container,
            context.memories,
            context.entities,
            context.resolutions,
            context.graph,
            context.relationships,
            survivor,
            retired,
            timestamp,
        )
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if outcome.changed {
            if decision.deterministic {
                deterministic_merges += 1;
            } else {
                model_merges += 1;
            }
        }
    }

    let mut wake_keys = Vec::new();
    for key in prepared.late_rejected.iter().copied() {
        let Some(state) = context.resolutions.get(key).cloned() else {
            continue;
        };
        if !matches!(state.status, MemoryEntityResolutionStatus::Rejected { .. }) {
            continue;
        }
        let entities = context.entities.current();
        let Some(metadata) = context.memories.routing_metadata(key.memory_id) else {
            continue;
        };
        let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
            mention.field == key.field
                && mention.start_byte == key.start_byte
                && mention.end_byte == key.end_byte
        }) else {
            continue;
        };
        let Some(entity) = exact_entity_for_surface(&entities, &mention.text) else {
            continue;
        };
        if entity.created_at_ns <= state.updated_at_ns {
            continue;
        }
        timestamp = timestamp.saturating_add(1);
        let mut owner = ResolutionWriteContext {
            container: context.container,
            memories: context.memories,
            entities: context.entities,
            entity_resolutions: context.resolutions,
        };
        owner
            .reset_entity_resolution(key, timestamp)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        wake_keys.push(key);
    }
    context
        .container
        .sync()
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    let report = EntityReconciliationReport {
        rounds: 1,
        candidate_pairs: prepared.pairs.len(),
        deterministic_merges,
        model_merges,
        rejected_mentions_reconsidered: wake_keys.len(),
        unresolved_pairs: prepared
            .pairs
            .len()
            .saturating_sub(deterministic_merges + model_merges),
        final_audit: crate::entity_audit::audit_owner(&crate::entity_audit::EntityAuditContext {
            entities: context.entities,
            memories: context.memories,
            graph: context.graph,
            resolutions: context.resolutions,
        }),
    };
    Ok(Some(EntityReconciliationCommit { report, wake_keys }))
}
