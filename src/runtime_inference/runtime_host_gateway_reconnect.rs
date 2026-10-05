//! P4.5 lifecycle publication after quiesced authoritative reconciliation.
use super::{GatewayRuntime, ReliquaryRuntimeHostError, operation};

impl GatewayRuntime {
    /// Caller owns the gateway lifecycle mutex; the REL remains drained and
    /// workers stopped. Failed restart closes the new distribution again.
    pub(super) fn reopen_rel_distribution(
        &self,
        owner_id: &str,
        epoch: u64,
        sequencer: &super::events::RelSequencer,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut watermarks = self.with_quiesced_rel_during_transition(owner_id, |runtime| {
            runtime.cva.sync().map_err(operation)?;
            Ok(super::snapshots::reader::source_watermarks(runtime.cva()))
        })?;
        watermarks.conversation_journal_sequence = Some(
            self.conversation
                .journal(owner_id)?
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .sequence(),
        );
        sequencer.reset_epoch(epoch)?;
        sequencer.gate(epoch)?.publish(
            super::events::RelEventClass::OwnerLifecycleChanged,
            watermarks,
            vec![super::events::RelInvalidation::LifecycleOpen],
        )?;
        Ok(())
    }
}
