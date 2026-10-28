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

pub(crate) fn merge_temporal_analysis(base: &mut TemporalAnalysis, mut extra: TemporalAnalysis) {
    extra.anchors.retain(|candidate| {
        !base.anchors.iter().any(|existing| {
            existing.start_ns == candidate.start_ns
                && existing.end_ns == candidate.end_ns
                && existing.granularity == candidate.granularity
                && existing.origin == candidate.origin
        })
    });
    extra.times_of_day.retain(|candidate| {
        !base.times_of_day.iter().any(|existing| {
            existing.hour == candidate.hour
                && existing.minute == candidate.minute
                && existing.second == candidate.second
                && existing.precision == candidate.precision
                && existing.origin == candidate.origin
        })
    });
    extra.durations.retain(|candidate| {
        !base
            .durations
            .iter()
            .any(|existing| existing.amount == candidate.amount && existing.unit == candidate.unit)
    });
    extra.duration_ranges.retain(|candidate| {
        !base.duration_ranges.iter().any(|existing| {
            existing.min_amount == candidate.min_amount
                && existing.max_amount == candidate.max_amount
                && existing.unit == candidate.unit
        })
    });
    extra.approximate_durations.retain(|candidate| {
        !base.approximate_durations.iter().any(|existing| {
            existing.amount == candidate.amount
                && existing.approximation == candidate.approximation
                && existing.unit == candidate.unit
        })
    });
    extra.event_relations.retain(|candidate| {
        !base.event_relations.iter().any(|existing| {
            existing.amount == candidate.amount
                && existing.unit == candidate.unit
                && existing.direction == candidate.direction
                && existing.reference_event == candidate.reference_event
        })
    });
    extra.intervals.retain(|candidate| {
        !base.intervals.iter().any(|existing| {
            existing.start_ns == candidate.start_ns
                && existing.end_ns == candidate.end_ns
                && existing.start_granularity == candidate.start_granularity
                && existing.end_granularity == candidate.end_granularity
                && existing.origin == candidate.origin
        })
    });
    extra.patterns.retain(|candidate| {
        !base.patterns.iter().any(|existing| {
            existing.frequency == candidate.frequency
                && existing.interval == candidate.interval
                && existing.occurrences_per_period == candidate.occurrences_per_period
                && existing.weekday == candidate.weekday
                && existing.month_day == candidate.month_day
                && existing.month == candidate.month
        })
    });
    base.anchors.append(&mut extra.anchors);
    base.times_of_day.append(&mut extra.times_of_day);
    base.durations.append(&mut extra.durations);
    base.duration_ranges.append(&mut extra.duration_ranges);
    base.approximate_durations
        .append(&mut extra.approximate_durations);
    base.event_relations.append(&mut extra.event_relations);
    base.intervals.append(&mut extra.intervals);
    base.patterns.append(&mut extra.patterns);
    crate::chronos_normalize::anchors(&mut base.anchors);
    crate::chronos_normalize::times_of_day(&mut base.times_of_day);
    crate::chronos_normalize::durations(&mut base.durations);
    crate::chronos_normalize::duration_ranges(&mut base.duration_ranges);
    crate::chronos_normalize::approximate_durations(&mut base.approximate_durations);
    crate::chronos_normalize::event_relations(&mut base.event_relations);
    crate::chronos_normalize::intervals(&mut base.intervals);
    crate::chronos_normalize::patterns(&mut base.patterns);
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
