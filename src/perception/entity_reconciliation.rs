use crate::entity_reconciliation_candidates::reconciliation_candidates;
use crate::entity_reconciliation_inference::classify_pair;
use crate::entity_reconciliation_support::{choose_survivor, merge_alias_budget_fits};
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{
    EntityId, EntityReconciliationCandidate, EntityReconciliationError,
    EntityReconciliationRelation, GeneralEndpoint, Memory,
};
use std::collections::{HashMap, HashSet};

pub(crate) struct ReconciliationRoundPlan {
    pub(crate) candidates: Vec<EntityReconciliationCandidate>,
    pub(crate) original_degrees: HashMap<EntityId, usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconciliationCandidateAction {
    Skip,
    Unresolved,
    Merge {
        survivor: EntityId,
        retired: EntityId,
        deterministic: bool,
    },
}

#[derive(Default)]
pub(crate) struct ReconciliationRoundOutcome {
    pub(crate) candidate_pairs: usize,
    pub(crate) deterministic_merges: usize,
    pub(crate) model_merges: usize,
    pub(crate) unresolved_pairs: usize,
    pub(crate) changed: bool,
}

pub(crate) fn plan_reconciliation_round(
    entities: &EntityStore,
    graph: &GraphStore,
) -> ReconciliationRoundPlan {
    let original_entities = entities.current();
    let original_degrees = original_entities
        .iter()
        .map(|entity| (entity.id, graph.memories_for_entity(entity.id).len()))
        .collect::<HashMap<_, _>>();
    let candidates = reconciliation_candidates(
        &original_entities,
        crate::DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT,
    );
    ReconciliationRoundPlan {
        candidates,
        original_degrees,
    }
}

pub(crate) fn evaluate_reconciliation_candidate<E: GeneralEndpoint>(
    entities: &EntityStore,
    graph: &GraphStore,
    memories: &MemoryReadView<'_>,
    plan: &ReconciliationRoundPlan,
    candidate: &EntityReconciliationCandidate,
    endpoint: &E,
    seen_model_pairs: &mut HashSet<(EntityId, EntityId)>,
) -> Result<ReconciliationCandidateAction, EntityReconciliationError> {
    if entities.retired_replacement(candidate.left).is_some()
        || entities.retired_replacement(candidate.right).is_some()
    {
        return Ok(ReconciliationCandidateAction::Skip);
    }
    let left = entities
        .entity(candidate.left)
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    let right = entities
        .entity(candidate.right)
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    if left.kind != right.kind {
        return Ok(ReconciliationCandidateAction::Skip);
    }

    let should_merge = if candidate.deterministic {
        true
    } else if seen_model_pairs.insert((candidate.left, candidate.right)) {
        let left_support = reconciliation_support(graph, memories, left.id)?;
        let right_support = reconciliation_support(graph, memories, right.id)?;
        matches!(
            classify_pair(endpoint, &left, &right, &left_support, &right_support)?,
            EntityReconciliationRelation::SameIdentity
        )
    } else {
        false
    };
    if !should_merge {
        return Ok(if candidate.deterministic {
            ReconciliationCandidateAction::Skip
        } else {
            ReconciliationCandidateAction::Unresolved
        });
    }

    let (survivor, retired) = choose_survivor(&left, &right, &plan.original_degrees);
    let (survivor_entity, retired_entity) = if survivor == left.id {
        (&left, &right)
    } else {
        (&right, &left)
    };
    if !merge_alias_budget_fits(survivor_entity, retired_entity) {
        return Ok(ReconciliationCandidateAction::Unresolved);
    }
    Ok(ReconciliationCandidateAction::Merge {
        survivor,
        retired,
        deterministic: candidate.deterministic,
    })
}

fn reconciliation_support(
    graph: &GraphStore,
    memories: &MemoryReadView<'_>,
    entity_id: EntityId,
) -> Result<Vec<Memory>, EntityReconciliationError> {
    graph
        .memories_for_entity(entity_id)
        .into_iter()
        .take(3)
        .map(|id| {
            memories
                .memory(id)
                .map_err(|error| EntityReconciliationError::Operation(error.to_string()))
        })
        .collect()
}
