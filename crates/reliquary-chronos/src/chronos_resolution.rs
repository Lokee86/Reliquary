use crate::{
    TemporalAnalysis, TemporalDetection, TemporalIndication, TemporalResolution,
    TemporalResolutionStatus,
};

pub(crate) fn assess_resolution(
    text: &str,
    detection: &TemporalDetection,
    analysis: &TemporalAnalysis,
) -> TemporalResolution {
    let resolved_spans = resolved_spans(text, analysis);
    let unresolved_indications = detection
        .indications
        .iter()
        .filter(|indication| !indication_resolved(indication, &resolved_spans))
        .cloned()
        .collect::<Vec<_>>();

    let status = if detection.indications.is_empty() && !has_analysis_output(analysis) {
        TemporalResolutionStatus::NoTemporalMaterial
    } else if unresolved_indications.is_empty() {
        TemporalResolutionStatus::FullyResolved
    } else {
        TemporalResolutionStatus::Unresolved
    };

    TemporalResolution {
        status,
        unresolved_indications,
    }
}

fn resolved_spans(text: &str, analysis: &TemporalAnalysis) -> Vec<(usize, usize)> {
    let evidence = analysis
        .anchors
        .iter()
        .map(|value| value.evidence.as_str())
        .chain(
            analysis
                .times_of_day
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .durations
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .duration_ranges
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .approximate_durations
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .event_relations
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .intervals
                .iter()
                .map(|value| value.evidence.as_str()),
        )
        .chain(
            analysis
                .patterns
                .iter()
                .map(|value| value.evidence.as_str()),
        );

    let mut spans = evidence
        .flat_map(|evidence| {
            text.match_indices(evidence)
                .map(move |(start, _)| (start, start + evidence.len()))
        })
        .collect::<Vec<_>>();
    spans.sort_unstable();
    spans.dedup();
    spans
}

fn indication_resolved(indication: &TemporalIndication, spans: &[(usize, usize)]) -> bool {
    spans
        .iter()
        .any(|&(start, end)| start <= indication.start_byte && indication.end_byte <= end)
}

fn has_analysis_output(analysis: &TemporalAnalysis) -> bool {
    !(analysis.anchors.is_empty()
        && analysis.times_of_day.is_empty()
        && analysis.durations.is_empty()
        && analysis.duration_ranges.is_empty()
        && analysis.approximate_durations.is_empty()
        && analysis.event_relations.is_empty()
        && analysis.intervals.is_empty()
        && analysis.patterns.is_empty())
}
