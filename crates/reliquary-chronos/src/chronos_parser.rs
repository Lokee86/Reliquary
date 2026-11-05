use crate::{TemporalAnalysis, TemporalAnchor, TemporalInterval};

#[derive(Clone, Copy)]
pub(crate) struct TextSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn parse_temporal(text: &str, source_timestamp_ns: Option<i64>) -> TemporalAnalysis {
    let mut claimed = Vec::new();
    let mut anchors = Vec::new();
    let mut intervals = Vec::new();
    crate::chronos_boundary::extract_boundaries(
        text,
        source_timestamp_ns,
        &mut claimed,
        &mut anchors,
        &mut intervals,
    );
    crate::chronos_absolute::extract_absolute(text, &mut claimed, &mut anchors);
    crate::chronos_loose_calendar::extract_relative(
        text,
        source_timestamp_ns,
        &mut claimed,
        &mut anchors,
    );
    crate::chronos_relative::extract_relative(
        text,
        source_timestamp_ns,
        &mut claimed,
        &mut anchors,
    );
    mirror_range_anchors(&anchors, &mut intervals);
    let mut times_of_day = crate::chronos_time_of_day::extract_times_of_day(text);
    let mut durations = crate::chronos_duration::extract_durations(text);
    let mut duration_ranges = crate::chronos_duration::extract_duration_ranges(text);
    let mut approximate_durations = crate::chronos_duration::extract_approximate_durations(text);
    let mut event_relations = crate::chronos_event_relative::extract_event_relations(text);
    let mut patterns = crate::chronos_recurrence::extract_recurrence(text);
    crate::chronos_normalize::anchors(&mut anchors);
    crate::chronos_normalize::times_of_day(&mut times_of_day);
    crate::chronos_normalize::durations(&mut durations);
    crate::chronos_normalize::duration_ranges(&mut duration_ranges);
    crate::chronos_normalize::approximate_durations(&mut approximate_durations);
    crate::chronos_normalize::event_relations(&mut event_relations);
    crate::chronos_normalize::intervals(&mut intervals);
    crate::chronos_normalize::patterns(&mut patterns);
    TemporalAnalysis {
        source_timestamp_ns,
        anchors,
        times_of_day,
        durations,
        duration_ranges,
        approximate_durations,
        event_relations,
        intervals,
        patterns,
    }
}

pub(crate) fn claimed(span: TextSpan, claimed: &[TextSpan]) -> bool {
    claimed
        .iter()
        .any(|other| span.start < other.end && other.start < span.end)
}

pub(crate) fn push_claimed(claimed_spans: &mut Vec<TextSpan>, span: TextSpan) {
    claimed_spans.push(span);
}

fn mirror_range_anchors(anchors: &[TemporalAnchor], intervals: &mut Vec<TemporalInterval>) {
    for anchor in anchors
        .iter()
        .filter(|anchor| anchor.granularity == crate::TemporalGranularity::Range)
    {
        if intervals.iter().any(|interval| {
            interval.start_ns == Some(anchor.start_ns)
                && interval.end_ns == Some(anchor.end_ns)
                && interval.origin == anchor.origin
                && interval.evidence == anchor.evidence
        }) {
            continue;
        }
        intervals.push(TemporalInterval {
            start_ns: Some(anchor.start_ns),
            end_ns: Some(anchor.end_ns),
            start_granularity: Some(crate::TemporalGranularity::Range),
            end_granularity: Some(crate::TemporalGranularity::Range),
            origin: anchor.origin,
            evidence: anchor.evidence.clone(),
        });
    }
}

pub(crate) use crate::chronos_merge::merge_temporal_analysis;
