use crate::{Cva, DreamPublicationOutcome, MemoryId};

/// Capture the durable first-pass identity before any Dream relation is published.
/// The Freshness owner returns the original frozen cut on retry.
pub(crate) fn begin_initial_pass(
    cva: &mut Cva,
    source_id: MemoryId,
    _candidate_ids: impl Iterator<Item = MemoryId>,
) -> Result<Option<crate::InitialDreamPass>, crate::CvaError> {
    let _ = _candidate_ids;
    crate::freshness_operations_runtime::begin_initial_dream_pass(cva, source_id)
}

pub(crate) fn record_publication_roots(
    cva: &mut Cva,
    source_id: MemoryId,
    publication: &DreamPublicationOutcome,
) -> Result<(), crate::CvaError> {
    crate::freshness_operations_runtime::record_initial_dream_publication_roots(
        cva,
        source_id,
        publication,
    )
}

use crate::dream_settlement::{InitialSettlementResult, InitialSettlementWork};

impl InitialSettlementResult {
    pub(crate) fn commit(self, cva: &mut Cva) -> Result<(), crate::CvaError> {
        crate::freshness_operations_runtime::commit_initial_dream_settlement(cva, self)
    }
}
pub(crate) fn prepare_initial_settlement(
    cva: &mut Cva,
    source_id: MemoryId,
    active_admission: bool,
) -> Result<Option<InitialSettlementWork>, crate::CvaError> {
    crate::freshness_operations_runtime::prepare_initial_dream_settlement(
        cva,
        source_id,
        active_admission,
    )
}
#[cfg(feature = "freshness-performance-trace")]
impl Cva {
    /// Diagnostic-only adapter for replaying the real pending initial-Dream
    /// prepare/evaluate/commit path from a disposable fixture.
    pub fn freshness_trace_run_initial_dream_fixture(
        &mut self,
        source_id: MemoryId,
        preexisting_roots: &[MemoryId],
        workers: usize,
    ) -> Result<bool, crate::CvaError> {
        crate::freshness_operations_runtime::run_initial_dream_trace_fixture(
            self,
            source_id,
            preexisting_roots,
            workers,
        )
    }

    pub fn freshness_trace_flush(&self) {
        crate::freshness_operations_runtime::flush_freshness_trace();
    }
}

pub(crate) fn settle_initial_pass(
    cva: &mut Cva,
    source_id: MemoryId,
    active_admission: bool,
    workers: usize,
) -> Result<(), crate::CvaError> {
    crate::freshness_operations_runtime::settle_initial_dream_pass(
        cva,
        source_id,
        active_admission,
        workers,
    )
}
