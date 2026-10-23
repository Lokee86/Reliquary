pub(crate) use crate::relationship_ordered_replay::{RelationshipReplayResult, RelationshipTail};
use crate::{Cva, CvaReconcileError};
pub(crate) fn read_relationship_tail(
    owner: &mut Cva,
    start: usize,
) -> Result<RelationshipTail, CvaReconcileError> {
    read_relationship_range(owner, start, usize::MAX)
}
pub(crate) fn read_relationship_range(
    owner: &mut Cva,
    start: usize,
    end: usize,
) -> Result<RelationshipTail, CvaReconcileError> {
    crate::relationship_ordered_replay::read_relationship_range(
        &owner.container,
        &owner.relationships,
        start,
        end,
    )
    .map_err(Into::into)
}
pub(crate) fn replay_relationship_tail(
    owner: &mut Cva,
    tail: RelationshipTail,
) -> Result<RelationshipReplayResult, CvaReconcileError> {
    crate::relationship_ordered_replay::replay_relationship_tail(
        &mut owner.container,
        &mut owner.relationships,
        tail,
    )
    .map_err(Into::into)
}

#[cfg(test)]
#[path = "cva_reconcile_relationship_compact_tests.rs"]
mod compact_tests;

#[cfg(test)]
#[path = "cva_reconcile_relationship_envelope_tests.rs"]
mod envelope_tests;
