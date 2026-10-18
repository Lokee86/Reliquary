use crate::cva_reconcile::with_replayed_transaction_time;
use crate::graph_codec::{decode_batch, decode_mutation, decode_version};
use crate::{
    Cva, CvaReconcileConflict, CvaReconcileError, GraphRelationOrigin, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GraphKey {
    source: SemanticNodeRef,
    target: SemanticNodeRef,
    kind: SemanticGraphRelationKind,
}

#[derive(Clone, Debug)]
struct GraphTailTransaction {
    changes: Vec<SemanticGraphRelationChange>,
    origin: GraphRelationOrigin,
    transaction_time_ns: Option<i64>,
}

pub(crate) struct GraphTail {
    transactions: Vec<GraphTailTransaction>,
}

#[cfg(test)]
impl GraphTail {
    pub(crate) fn for_test(transactions: Vec<Vec<crate::GraphRelationChange>>) -> Self {
        Self {
            transactions: transactions
                .into_iter()
                .map(|changes| GraphTailTransaction {
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

pub(crate) fn read_graph_tail(
    cva: &mut Cva,
    start_chunk: usize,
) -> Result<GraphTail, CvaReconcileError> {
    read_graph_range(cva, start_chunk, usize::MAX)
}

pub(crate) fn read_graph_range(
    cva: &mut Cva,
    start_chunk: usize,
    end_chunk: usize,
) -> Result<GraphTail, CvaReconcileError> {
    let chunks = cva.container.chunks()?;
    let mut transactions = Vec::new();
    for chunk in chunks.iter().take(end_chunk).skip(start_chunk) {
        let payload = cva.container.read(*chunk)?;
        let Some(version) = decode_version(&payload)? else {
            continue;
        };
        let mutation_payload = cva.container.read(version.mutation)?;
        let mutations = if let Some(batch) = decode_batch(&mutation_payload)? {
            batch
        } else if let Some(mutation) = decode_mutation(&mutation_payload)? {
            vec![mutation]
        } else {
            return Err(CvaReconcileError::InvalidGraphVersionRecord);
        };
        let origin = mutations
            .first()
            .map(|mutation| mutation.origin)
            .ok_or(CvaReconcileError::InvalidGraphVersionRecord)?;
        if mutations.iter().any(|mutation| mutation.origin != origin) {
            return Err(CvaReconcileError::InvalidGraphVersionRecord);
        }
        transactions.push(GraphTailTransaction {
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
            transaction_time_ns: cva.transaction_time_ns(version.global_version),
        });
    }
    Ok(GraphTail { transactions })
}

pub(crate) fn replay_graph_tail(
    destination: &mut Cva,
    tail: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    replay_with_skips(destination, tail, &HashMap::new())
}

pub(crate) fn reconcile_right_graph_tail(
    destination: &mut Cva,
    left_divergent: &GraphTail,
    right_divergent: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    let skips = replay_skips(left_divergent, right_divergent)?;
    replay_with_skips(destination, right_divergent, &skips)
}

#[cfg(test)]
pub(crate) fn validate_prefix_for_test(
    left: &GraphTail,
    right: &GraphTail,
) -> Result<(), CvaReconcileError> {
    replay_skips(left, right).map(|_| ())
}

pub(crate) fn validate_graph_tails(
    left: &GraphTail,
    right: &GraphTail,
) -> Result<(), CvaReconcileError> {
    replay_skips(left, right).map(|_| ())
}

fn replay_skips(
    left: &GraphTail,
    right: &GraphTail,
) -> Result<HashMap<GraphKey, usize>, CvaReconcileError> {
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
            return Err(CvaReconcileError::Conflict(conflict_for(
                key,
                left_states,
                &right_states,
            )));
        }
        if common > 0 {
            skips.insert(key, common);
        }
    }
    Ok(skips)
}

fn replay_with_skips(
    destination: &mut Cva,
    tail: &GraphTail,
    skips: &HashMap<GraphKey, usize>,
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
            if *occurrence < skips.get(&key).copied().unwrap_or(0) {
                duplicate_mutations += 1;
            } else {
                changes.push(*change);
            }
            *occurrence += 1;
        }
        if changes.is_empty() {
            continue;
        }
        let graph_version = destination.graph_version();
        let published = with_replayed_transaction_time(
            destination,
            transaction.transaction_time_ns,
            |destination| {
                destination.set_semantic_relations_with_origin(
                    &changes,
                    transaction.origin,
                    graph_version,
                )
            },
        )?;
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

#[cfg(test)]
fn memory_change(change: crate::GraphRelationChange) -> SemanticGraphRelationChange {
    SemanticGraphRelationChange {
        source: SemanticNodeRef::memory(change.source),
        target: SemanticNodeRef::memory(change.target),
        kind: SemanticGraphRelationKind::Memory(change.kind),
        active: change.active,
    }
}

fn conflict_for(
    key: GraphKey,
    left: &[(bool, GraphRelationOrigin)],
    right: &[(bool, GraphRelationOrigin)],
) -> CvaReconcileConflict {
    if let (Some(source), Some(target), SemanticGraphRelationKind::Memory(relation_kind)) =
        (key.source.as_memory(), key.target.as_memory(), key.kind)
    {
        return CvaReconcileConflict::GraphRelation {
            source,
            target,
            relation_kind,
            left_states: left.iter().map(|(active, _)| *active).collect(),
            right_states: right.iter().map(|(active, _)| *active).collect(),
        };
    }
    CvaReconcileConflict::SemanticGraphRelation {
        source: key.source,
        target: key.target,
        relation_kind: key.kind,
        left_states: left.iter().map(|(active, _)| *active).collect(),
        right_states: right.iter().map(|(active, _)| *active).collect(),
    }
}
