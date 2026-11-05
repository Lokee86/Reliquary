//! Validate stored body/time-bound inference through pure Chronos.
use crate::DreamWorkflowError;
use reliquary_owner_session::model::{MemoryBody, MemoryRecord};
pub(crate) fn analyze(
    record: &MemoryRecord,
    body: &MemoryBody,
) -> Result<reliquary_chronos::TemporalAnalysis, DreamWorkflowError> {
    if reliquary_owner_session::model::memory_body_id(&body.title, &body.content) != record.body_id
    {
        return Err(DreamWorkflowError::TemporalInferenceUnavailable);
    }
    let text = format!("{}\n{}", body.title, body.content);
    let Some(value) = &record.temporal_inference else {
        return Ok(reliquary_chronos::analyze(&text, record.source_time_ns));
    };
    if value.body_id != record.body_id || value.source_time_ns != record.source_time_ns {
        return Err(DreamWorkflowError::TemporalInferenceUnavailable);
    }
    use reliquary_chronos::TemporalIndicationKind as Kind;
    use reliquary_owner_session::model::TemporalIndicationKind as Stored;
    let inference = reliquary_chronos::TemporalInference {
        model: value.inference.model.clone(),
        contract_version: value.inference.contract_version.clone(),
        resolutions: value
            .inference
            .resolutions
            .iter()
            .map(|r| reliquary_chronos::TemporalInferenceResolution {
                start_byte: r.start_byte,
                end_byte: r.end_byte,
                evidence: r.evidence.clone(),
                canonical_expression: r.canonical_expression.clone(),
                kind: match r.kind {
                    Stored::Explicit => Kind::Explicit,
                    Stored::Calendar => Kind::Calendar,
                    Stored::Relative => Kind::Relative,
                    Stored::Boundary => Kind::Boundary,
                    Stored::Recurrence => Kind::Recurrence,
                    Stored::Duration => Kind::Duration,
                },
            })
            .collect(),
    };
    reliquary_chronos::analyze_with_inference(&text, record.source_time_ns, &inference)
        .map_err(|_| DreamWorkflowError::TemporalInferenceUnavailable)
}
