use crate::TemporalAnalysis;
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
