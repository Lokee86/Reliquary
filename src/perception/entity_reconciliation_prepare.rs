use crate::entity_reconciliation_candidates::reconciliation_candidates;
use crate::entity_reconciliation_model::{
    EntityReconciliationPrepared, EntityReconciliationPreparedPair,
};
use crate::entity_reconciliation_support::exact_entity_for_surface;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{
    Container, DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT, EntityReconciliationError,
    MemoryEntityResolutionStatus,
};

pub(crate) struct ReconciliationPrepareContext<'a> {
    pub(crate) entities: &'a EntityStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) resolutions: &'a EntityResolutionStore,
    pub(crate) container: &'a Container,
}

pub(crate) fn prepare_entity_reconciliation(
    context: &ReconciliationPrepareContext<'_>,
) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
    let entities = context.entities.current();
    let candidates = reconciliation_candidates(&entities, DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT);
    let mut pairs = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let left = context
            .entities
            .entity(candidate.left)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        let right = context
            .entities
            .entity(candidate.right)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
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
        pairs.push(EntityReconciliationPreparedPair {
            candidate,
            left,
            right,
            left_support,
            right_support,
        });
    }
    let mut late_rejected = Vec::new();
    for state in context.resolutions.records() {
        if !matches!(state.status, MemoryEntityResolutionStatus::Rejected { .. }) {
            continue;
        }
        let Some(metadata) = context.memories.routing_metadata(state.key.memory_id) else {
            continue;
        };
        let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
            mention.field == state.key.field
                && mention.start_byte == state.key.start_byte
                && mention.end_byte == state.key.end_byte
        }) else {
            continue;
        };
        let Some(entity) = exact_entity_for_surface(&entities, &mention.text) else {
            continue;
        };
        if entity.created_at_ns > state.updated_at_ns {
            late_rejected.push(state.key);
        }
    }
    Ok(EntityReconciliationPrepared {
        entity_version: context.entities.entity_version(),
        graph_version: context.graph.graph_version(),
        memory_version: context.memories.memory_version(),
        pairs,
        late_rejected,
    })
}
