use crate::graph_ordered_replay::{GraphKey, GraphTailReadError};
pub(crate) use crate::graph_ordered_replay::{GraphReplayResult, GraphTail};
use crate::{
    Cva, CvaReconcileConflict, CvaReconcileError, GraphRelationOrigin, SemanticGraphRelationChange,
    SemanticGraphRelationKind,
};
use std::collections::HashMap;
#[cfg(test)]
#[path = "graph_compact_reconcile_tests.rs"]
mod compact_tests;
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
    crate::graph_ordered_replay::read_graph_range(
        &cva.container,
        &cva.graph,
        start_chunk,
        end_chunk,
    )
    .map_err(|error| match error {
        GraphTailReadError::Graph(error) => error.into(),
        GraphTailReadError::InvalidGraphVersionRecord => {
            CvaReconcileError::InvalidGraphVersionRecord
        }
    })
}
pub(crate) fn replay_graph_tail(
    destination: &mut Cva,
    tail: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    replay_with_skips(destination, tail, &HashMap::new(), false)
}

pub(crate) fn replay_import_graph_tail(
    destination: &mut Cva,
    tail: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    replay_with_skips(destination, tail, &HashMap::new(), true)
}

pub(crate) fn reconcile_right_graph_tail(
    destination: &mut Cva,
    left_divergent: &GraphTail,
    right_divergent: &GraphTail,
) -> Result<GraphReplayResult, CvaReconcileError> {
    let skips = replay_skips(left_divergent, right_divergent)?;
    replay_with_skips(destination, right_divergent, &skips, false)
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
    crate::graph_ordered_replay::replay_skips(left, right)
        .map_err(crate::owner_lifecycle::graph_ordered_replay::conflict_error)
}
fn replay_with_skips(
    destination: &mut Cva,
    tail: &GraphTail,
    skips: &HashMap<GraphKey, usize>,
    preserve_import_postimages: bool,
) -> Result<GraphReplayResult, CvaReconcileError> {
    crate::owner_lifecycle::graph_ordered_replay::replay_with_skips(
        crate::owner_lifecycle::graph_ordered_replay::GraphReplayDestination {
            container: &mut destination.owner.container,
            memories: &destination.owner.memories,
            entities: &destination.owner.entities,
            graph: &mut destination.owner.graph,
        },
        tail,
        skips,
        preserve_import_postimages,
    )
}
