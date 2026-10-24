use crate::entity_reconciliation_candidates::reconciliation_candidates;
use crate::entity_reconciliation_inference::classify_pair;
use crate::entity_reconciliation_support::{choose_survivor, merge_alias_budget_fits};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Container, EntityId, EntityReconciliationError, EntityReconciliationRelation, GeneralEndpoint,
};
use std::collections::{HashMap, HashSet};

pub(crate) struct ReconciliationRoundContext<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) graph: &'a mut GraphStore,
    pub(crate) resolutions: &'a mut EntityResolutionStore,
    pub(crate) relationships: &'a mut RelationshipStore,
}

#[derive(Default)]
pub(crate) struct ReconciliationRoundOutcome {
    pub(crate) candidate_pairs: usize,
    pub(crate) deterministic_merges: usize,
    pub(crate) model_merges: usize,
    pub(crate) unresolved_pairs: usize,
    pub(crate) changed: bool,
}

pub(crate) fn reconcile_round<E: GeneralEndpoint>(
    context: &mut ReconciliationRoundContext<'_>,
    endpoint: &E,
    timestamp: &mut i64,
    seen_model_pairs: &mut HashSet<(EntityId, EntityId)>,
) -> Result<ReconciliationRoundOutcome, EntityReconciliationError> {
    let mut outcome = ReconciliationRoundOutcome::default();
    let entities = context.entities.current();
    let degrees = entities
        .iter()
        .map(|entity| {
            (
                entity.id,
                context.graph.memories_for_entity(entity.id).len(),
            )
        })
        .collect::<HashMap<_, _>>();
    let candidates =
        reconciliation_candidates(&entities, crate::DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT);
    outcome.candidate_pairs = candidates.len();
    for candidate in candidates {
        if context
            .entities
            .retired_replacement(candidate.left)
            .is_some()
            || context
                .entities
                .retired_replacement(candidate.right)
                .is_some()
        {
            continue;
        }
        let left = context
            .entities
            .entity(candidate.left)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        let right = context
            .entities
            .entity(candidate.right)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if left.kind != right.kind {
            continue;
        }
        let should_merge = if candidate.deterministic {
            true
        } else if seen_model_pairs.insert((candidate.left, candidate.right)) {
            let left_support = context
                .graph
                .memories_for_entity(left.id)
                .into_iter()
                .take(3)
                .map(|id| {
                    context
                        .memories
                        .memory(context.container, id)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let right_support = context
                .graph
                .memories_for_entity(right.id)
                .into_iter()
                .take(3)
                .map(|id| {
                    context
                        .memories
                        .memory(context.container, id)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            matches!(
                classify_pair(endpoint, &left, &right, &left_support, &right_support)?,
                EntityReconciliationRelation::SameIdentity
            )
        } else {
            false
        };
        if !should_merge {
            if !candidate.deterministic {
                outcome.unresolved_pairs += 1;
            }
            continue;
        }
        let (survivor, retired) = choose_survivor(&left, &right, &degrees);
        let (survivor_entity, retired_entity) = if survivor == left.id {
            (&left, &right)
        } else {
            (&right, &left)
        };
        if !merge_alias_budget_fits(survivor_entity, retired_entity) {
            outcome.unresolved_pairs += 1;
            continue;
        }
        *timestamp = timestamp.saturating_add(1);
        let merged = crate::entity_merge::merge_entities_parts::<crate::CvaError>(
            context.container,
            context.memories,
            context.entities,
            context.resolutions,
            context.graph,
            context.relationships,
            survivor,
            retired,
            *timestamp,
        )
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if merged.changed {
            outcome.changed = true;
            if candidate.deterministic {
                outcome.deterministic_merges += 1;
            } else {
                outcome.model_merges += 1;
            }
        }
    }
    Ok(outcome)
}
