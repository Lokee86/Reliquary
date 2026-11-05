use crate::{
    TemporalAnalysis, TemporalIndicationKind, TemporalInferenceError, TemporalResolutionStatus,
};

pub fn verify_canonical_expression(
    expression: &str,
    kind: TemporalIndicationKind,
    reference_timestamp_ns: Option<i64>,
) -> Result<TemporalAnalysis, TemporalInferenceError> {
    let verified = crate::assess(expression, reference_timestamp_ns);
    if verified.resolution.status != TemporalResolutionStatus::FullyResolved
        || !analysis_matches_kind(kind, &verified.analysis)
    {
        return Err(TemporalInferenceError::InvalidOutput(
            "canonical expression is not deterministically resolvable as the source temporal kind"
                .into(),
        ));
    }
    Ok(verified.analysis)
}

fn analysis_matches_kind(kind: TemporalIndicationKind, analysis: &TemporalAnalysis) -> bool {
    match kind {
        TemporalIndicationKind::Explicit | TemporalIndicationKind::Calendar => {
            !analysis.anchors.is_empty()
        }
        TemporalIndicationKind::Relative => {
            !analysis.anchors.is_empty() || !analysis.intervals.is_empty()
        }
        TemporalIndicationKind::Boundary => {
            !analysis.intervals.is_empty() || !analysis.event_relations.is_empty()
        }
        TemporalIndicationKind::Recurrence => !analysis.patterns.is_empty(),
        TemporalIndicationKind::Duration => {
            !analysis.durations.is_empty()
                || !analysis.duration_ranges.is_empty()
                || !analysis.approximate_durations.is_empty()
                || !analysis.event_relations.is_empty()
        }
    }
}
