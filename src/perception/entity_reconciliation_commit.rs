use crate::entity_merge::EntityMergePlan;
use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::entity_reconciliation_support::{
    choose_survivor, exact_entity_for_surface, merge_alias_budget_fits,
};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{
    EntityId, EntityReconciliationError, EntityReconciliationRelation, MemoryEntityMentionKey,
    MemoryEntityResolutionStatus,
};
use std::collections::HashMap;

pub(crate) struct ReconciliationCommitContext<'a, 'm> {
    pub(crate) memories: &'a MemoryReadView<'m>,
    pub(crate) entities: &'a EntityStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) resolutions: &'a EntityResolutionStore,
}

pub(crate) struct ReconciliationCommitPlan {
    pub(crate) candidate_pairs: usize,
    pub(crate) decisions: Vec<crate::entity_reconciliation_model::EntityReconciliationPairDecision>,
    pub(crate) late_rejected: Vec<MemoryEntityMentionKey>,
    pub(crate) original_degrees: HashMap<EntityId, usize>,
}

pub(crate) struct RejectionWakePlan {
    pub(crate) key: MemoryEntityMentionKey,
}

pub(crate) fn plan_entity_reconciliation_commit(
    context: &ReconciliationCommitContext<'_, '_>,
    prepared: EntityReconciliationPrepared,
    evaluation: EntityReconciliationEvaluation,
) -> Option<ReconciliationCommitPlan> {
    if context.entities.entity_version() != prepared.entity_version
        || context.graph.graph_version() != prepared.graph_version
        || context.memories.memory_version() != prepared.memory_version
    {
        return None;
    }
    let original_degrees = context
        .entities
        .current()
        .into_iter()
        .map(|entity| {
            (
                entity.id,
                context.graph.memories_for_entity(entity.id).len(),
            )
        })
        .collect::<HashMap<_, _>>();
    Some(ReconciliationCommitPlan {
        candidate_pairs: prepared.pairs.len(),
        decisions: evaluation.decisions,
        late_rejected: prepared.late_rejected,
        original_degrees,
    })
}

pub(crate) fn plan_reconciliation_merge(
    context: &ReconciliationCommitContext<'_, '_>,
    plan: &ReconciliationCommitPlan,
    decision: &crate::entity_reconciliation_model::EntityReconciliationPairDecision,
    now_ns: i64,
) -> Result<Option<(EntityMergePlan, bool)>, EntityReconciliationError> {
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
        return Ok(None);
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
        return Ok(None);
    }
    let (survivor_id, retired_id) = choose_survivor(&left, &right, &plan.original_degrees);
    let (survivor, retired) = if survivor_id == left.id {
        (&left, &right)
    } else {
        (&right, &left)
    };
    if !merge_alias_budget_fits(survivor, retired) {
        return Ok(None);
    }
    let merge_plan =
        crate::entity_merge::plan_entity_merge(context.entities, survivor_id, retired_id, now_ns)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    Ok(Some((merge_plan, decision.deterministic)))
}

pub(crate) fn plan_late_rejection_wake(
    context: &ReconciliationCommitContext<'_, '_>,
    key: MemoryEntityMentionKey,
) -> Option<RejectionWakePlan> {
    let state = context.resolutions.get(key).cloned()?;
    let MemoryEntityResolutionStatus::Rejected { .. } = state.status else {
        return None;
    };
    let metadata = context.memories.routing_metadata(key.memory_id)?;
    let mention = metadata.entity_mentions.iter().find(|mention| {
        mention.field == key.field
            && mention.start_byte == key.start_byte
            && mention.end_byte == key.end_byte
    })?;
    let entities = context.entities.current();
    let entity = exact_entity_for_surface(&entities, &mention.text)?;
    if entity.created_at_ns <= state.updated_at_ns {
        return None;
    }
    Some(RejectionWakePlan { key })
}
