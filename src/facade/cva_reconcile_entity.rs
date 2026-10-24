pub(crate) use crate::entity_ordered_replay::{EntityReplayResult, EntityTail};
use crate::{Cva, CvaReconcileError};
pub(crate) fn read_entity_tail(
    owner: &mut Cva,
    start: usize,
) -> Result<EntityTail, CvaReconcileError> {
    read_entity_range(owner, start, usize::MAX)
}
pub(crate) fn read_entity_range(
    owner: &mut Cva,
    start: usize,
    end: usize,
) -> Result<EntityTail, CvaReconcileError> {
    crate::entity_ordered_replay::read_entity_range(&owner.container, &owner.entities, start, end)
        .map_err(Into::into)
}
pub(crate) fn replay_entity_tail(
    owner: &mut Cva,
    tail: EntityTail,
) -> Result<EntityReplayResult, CvaReconcileError> {
    crate::entity_ordered_replay::replay_entity_tail(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        tail,
    )
    .map_err(Into::into)
}

#[cfg(test)]
#[path = "cva_reconcile_entity_compact_tests.rs"]
mod compact_tests;

#[cfg(test)]
#[path = "cva_reconcile_entity_envelope_tests.rs"]
mod envelope_tests;
