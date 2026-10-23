//! Validated Graph replay admission over explicit producer parts.
use crate::graph_ordered_replay::{GraphKey, GraphReplayResult, GraphTail};
use crate::{CvaReconcileError, SemanticGraphRelationChange, SemanticGraphRelationKind};
use std::collections::HashMap;
pub(crate) struct GraphReplayDestination<'a> {
    pub(crate) container: &'a mut crate::Container,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) entities: &'a crate::entity_store::EntityStore,
    pub(crate) graph: &'a mut crate::graph_store::GraphStore,
}
pub(crate) fn replay_with_skips(
    destination: GraphReplayDestination<'_>,
    tail: &GraphTail,
    skips: &HashMap<GraphKey, usize>,
    preserve_import_postimages: bool,
) -> Result<GraphReplayResult, CvaReconcileError> {
    let mut seen = HashMap::<GraphKey, usize>::new();
    let mut transactions = 0;
    let mut mutations = 0;
    let mut duplicate_mutations = 0;
    let mut source_memory_graph_changed = false;
    for transaction in &tail.transactions {
        source_memory_graph_changed |= transaction
            .changes
            .iter()
            .any(|change| matches!(change.kind, SemanticGraphRelationKind::Memory(_)));
        let mut changes = Vec::with_capacity(transaction.changes.len());
        for change in &transaction.changes {
            let key = key(*change);
            let occurrence = seen.entry(key).or_default();
            let duplicate = *occurrence < skips.get(&key).copied().unwrap_or(0);
            if duplicate {
                duplicate_mutations += 1;
            }
            if preserve_import_postimages || !duplicate {
                changes.push(*change);
            }
            *occurrence += 1;
        }
        if changes.is_empty() {
            continue;
        }
        crate::graph_endpoint_admission::validate_semantic_changes(
            destination.memories,
            destination.entities,
            &changes,
            transaction.origin,
        )?;
        let graph_version = destination.graph.graph_version();
        destination
            .container
            .set_next_transaction_time_override(transaction.transaction_time_ns);
        let published = if preserve_import_postimages {
            destination.graph.replay_import_semantic_transaction(
                destination.container,
                &changes,
                transaction.origin,
                graph_version,
                transaction.single,
            )
        } else {
            destination.graph.replay_semantic_transaction(
                destination.container,
                &changes,
                transaction.origin,
                graph_version,
                transaction.single,
            )
        };
        destination.container.clear_next_transaction_time_override();
        let published = published?;
        duplicate_mutations += changes.len().saturating_sub(published.len());
        if !published.is_empty() {
            transactions += 1;
            mutations += published.len();
        }
    }
    Ok(GraphReplayResult {
        transactions,
        mutations,
        duplicate_mutations,
        source_memory_graph_changed,
    })
}

fn key(change: SemanticGraphRelationChange) -> GraphKey {
    GraphKey {
        source: change.source,
        target: change.target,
        kind: change.kind,
    }
}

pub(crate) fn conflict_error(
    conflict: crate::graph_ordered_replay::GraphTailConflict,
) -> CvaReconcileError {
    let key = conflict.key;
    let conflict =
        if let (Some(source), Some(target), SemanticGraphRelationKind::Memory(relation_kind)) =
            (key.source.as_memory(), key.target.as_memory(), key.kind)
        {
            crate::CvaReconcileConflict::GraphRelation {
                source,
                target,
                relation_kind,
                left_states: conflict.left_states,
                right_states: conflict.right_states,
            }
        } else {
            crate::CvaReconcileConflict::SemanticGraphRelation {
                source: key.source,
                target: key.target,
                relation_kind: key.kind,
                left_states: conflict.left_states,
                right_states: conflict.right_states,
            }
        };
    CvaReconcileError::Conflict(conflict)
}
