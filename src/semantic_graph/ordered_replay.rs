//! Graph-owned ordered publication read contract.
use crate::{
    GraphRelationOrigin, SemanticGraphRelationChange, SemanticGraphRelationKind, SemanticNodeRef,
};
#[derive(Debug)]
pub(crate) enum GraphTailReadError {
    Graph(crate::GraphError),
    InvalidGraphVersionRecord,
}
impl From<crate::GraphError> for GraphTailReadError {
    fn from(error: crate::GraphError) -> Self {
        Self::Graph(error)
    }
}
impl From<crate::ContainerError> for GraphTailReadError {
    fn from(error: crate::ContainerError) -> Self {
        Self::Graph(error.into())
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct GraphKey {
    pub(crate) source: SemanticNodeRef,
    pub(crate) target: SemanticNodeRef,
    pub(crate) kind: SemanticGraphRelationKind,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphTailTransaction {
    pub(crate) single: bool,
    pub(crate) changes: Vec<SemanticGraphRelationChange>,
    pub(crate) origin: GraphRelationOrigin,
    pub(crate) transaction_time_ns: Option<i64>,
}

pub(crate) struct GraphTail {
    pub(crate) transactions: Vec<GraphTailTransaction>,
}

#[cfg(test)]
impl GraphTail {
    pub(crate) fn for_test(transactions: Vec<Vec<crate::GraphRelationChange>>) -> Self {
        Self {
            transactions: transactions
                .into_iter()
                .map(|changes| GraphTailTransaction {
                    single: changes.len() == 1,
                    changes: changes.into_iter().map(memory_change).collect(),
                    origin: GraphRelationOrigin::Dream,
                    transaction_time_ns: None,
                })
                .collect(),
        }
    }
}

pub(crate) struct GraphReplayResult {
    pub(crate) transactions: usize,
    pub(crate) mutations: usize,
    pub(crate) duplicate_mutations: usize,
    /// True when the source interval contains a Memory relation mutation,
    /// even if the destination already had the exact relation.
    pub(crate) source_memory_graph_changed: bool,
}

pub(crate) fn read_graph_range(
    container: &crate::Container,
    graph: &crate::graph_store::GraphStore,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<GraphTail, GraphTailReadError> {
    let chunks = container.chunk_refs()?;
    let mut transactions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = container.read(*chunk)?;
        let Some(version) = crate::graph_compact_codec::publication::version_at(*chunk, &payload)?
        else {
            continue;
        };
        let mutation_payload = container.read(version.mutation)?;
        let Some((single, mutations)) = graph.decode_stored_transaction(&mutation_payload)? else {
            return Err(GraphTailReadError::InvalidGraphVersionRecord);
        };
        let origin = mutations
            .first()
            .map(|mutation| mutation.origin)
            .ok_or(GraphTailReadError::InvalidGraphVersionRecord)?;
        if mutations.iter().any(|mutation| mutation.origin != origin) {
            return Err(GraphTailReadError::InvalidGraphVersionRecord);
        }
        transactions.push(GraphTailTransaction {
            single,
            changes: mutations
                .into_iter()
                .map(|mutation| SemanticGraphRelationChange {
                    source: mutation.source,
                    target: mutation.target,
                    kind: mutation.kind,
                    active: mutation.active,
                })
                .collect(),
            origin,
            transaction_time_ns: container.transaction_time_ns(version.global_version),
        });
    }
    Ok(GraphTail { transactions })
}

#[cfg(test)]
fn memory_change(change: crate::GraphRelationChange) -> SemanticGraphRelationChange {
    SemanticGraphRelationChange {
        source: SemanticNodeRef::memory(change.source),
        target: SemanticNodeRef::memory(change.target),
        kind: SemanticGraphRelationKind::Memory(change.kind),
        active: change.active,
    }
}

use std::collections::HashMap;
pub(crate) struct GraphTailConflict {
    pub(crate) key: GraphKey,
    pub(crate) left_states: Vec<bool>,
    pub(crate) right_states: Vec<bool>,
}
pub(crate) fn replay_skips(
    left: &GraphTail,
    right: &GraphTail,
) -> Result<HashMap<GraphKey, usize>, GraphTailConflict> {
    let left_sequences = sequences(left);
    let right_sequences = sequences(right);
    let mut skips = HashMap::new();
    for (key, right_states) in right_sequences {
        let Some(left_states) = left_sequences.get(&key) else {
            continue;
        };
        let common = left_states
            .iter()
            .zip(&right_states)
            .take_while(|(left, right)| left == right)
            .count();
        if common != left_states.len().min(right_states.len()) {
            return Err(GraphTailConflict {
                key,
                left_states: left_states.iter().map(|(active, _)| *active).collect(),
                right_states: right_states.iter().map(|(active, _)| *active).collect(),
            });
        }
        if common > 0 {
            skips.insert(key, common);
        }
    }
    Ok(skips)
}

fn sequences(tail: &GraphTail) -> HashMap<GraphKey, Vec<(bool, GraphRelationOrigin)>> {
    let mut sequences = HashMap::<GraphKey, Vec<(bool, GraphRelationOrigin)>>::new();
    for transaction in &tail.transactions {
        for change in &transaction.changes {
            sequences
                .entry(key(*change))
                .or_default()
                .push((change.active, transaction.origin));
        }
    }
    sequences
}

fn key(change: SemanticGraphRelationChange) -> GraphKey {
    GraphKey {
        source: change.source,
        target: change.target,
        kind: change.kind,
    }
}
