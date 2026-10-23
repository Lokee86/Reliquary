//! One bounded epoch replay scope. It cannot invoke a caller-supplied writer callback.
use crate::{Cva, CvaReconcileError};
pub(super) struct EpochReplay<'a> {
    destination: &'a mut Cva,
}
impl<'a> EpochReplay<'a> {
    pub(super) fn new(destination: &'a mut Cva) -> Self {
        crate::owner_lifecycle::import_epoch_transport::enter_epoch_write_scope(
            &mut destination.container,
        );
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
        crate::owner_lifecycle::import_epoch_transport::leave_epoch_write_scope(
            &mut self.destination.container,
        );
    }
}
