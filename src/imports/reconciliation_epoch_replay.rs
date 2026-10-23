//! One bounded epoch replay scope. It cannot invoke a caller-supplied writer callback.
use crate::{Cva, CvaReconcileError};
pub(super) struct EpochReplay<'a> {
    destination: &'a mut Cva,
}
impl<'a> EpochReplay<'a> {
    pub(super) fn new(destination: &'a mut Cva) -> Self {
        destination.container.set_owner_write_scope(true);
        Self { destination }
    }
    pub(super) fn replay(
        self,
        source: &mut Cva,
        first: usize,
        end: usize,
        coordinates: &mut super::reconciliation_interval::Coordinates,
    ) -> Result<usize, CvaReconcileError> {
        super::reconciliation_replay::replay_interval(
            source,
            self.destination,
            first,
            end,
            coordinates,
        )
    }
}
impl Drop for EpochReplay<'_> {
    fn drop(&mut self) {
        self.destination.container.set_owner_write_scope(false);
    }
}
