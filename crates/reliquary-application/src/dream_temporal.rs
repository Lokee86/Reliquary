//! Validate stored body/time-bound inference through the shared Chronos adapter.
use crate::DreamWorkflowError;
use reliquary_owner_session::model::{MemoryBody, MemoryRecord};
pub(crate) fn analyze(
    record: &MemoryRecord,
    body: &MemoryBody,
) -> Result<reliquary_chronos::TemporalAnalysis, DreamWorkflowError> {
    crate::temporal_search::analyze_memory(record, body)
        .map_err(|_| DreamWorkflowError::TemporalInferenceUnavailable)
}
