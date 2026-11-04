use crate::{
    TemporalAnchor, TemporalApproximateDuration, TemporalDuration, TemporalDurationRange,
    TemporalEventRelation, TemporalInterval, TemporalPattern, TemporalTimeOfDay,
};

pub(crate) fn anchors(values: &mut Vec<TemporalAnchor>) {
    values.sort_by(|left, right| {
        (
            left.start_ns,
            left.end_ns,
            left.granularity,
            left.origin,
            &left.evidence,
        )
            .cmp(&(
                right.start_ns,
                right.end_ns,
                right.granularity,
                right.origin,
                &right.evidence,
            ))
    });
    values.dedup_by(|left, right| {
        left.start_ns == right.start_ns
            && left.end_ns == right.end_ns
            && left.granularity == right.granularity
            && left.origin == right.origin
    });
}

pub(crate) fn times_of_day(values: &mut Vec<TemporalTimeOfDay>) {
    values.sort_by(|left, right| {
        (
            left.hour,
            left.minute,
            left.second,
            left.precision,
            left.origin,
            &left.evidence,
        )
            .cmp(&(
                right.hour,
                right.minute,
                right.second,
                right.precision,
                right.origin,
                &right.evidence,
            ))
    });
    values.dedup_by(|left, right| {
        left.hour == right.hour
            && left.minute == right.minute
            && left.second == right.second
            && left.precision == right.precision
            && left.origin == right.origin
    });
}

pub(crate) fn durations(values: &mut Vec<TemporalDuration>) {
    values.sort_by(|left, right| {
        (left.amount, left.unit, &left.evidence).cmp(&(right.amount, right.unit, &right.evidence))
    });
    values.dedup_by(|left, right| left.amount == right.amount && left.unit == right.unit);
}

pub(crate) fn duration_ranges(values: &mut Vec<TemporalDurationRange>) {
    values.sort_by(|left, right| {
        (left.min_amount, left.max_amount, left.unit, &left.evidence).cmp(&(
            right.min_amount,
            right.max_amount,
            right.unit,
            &right.evidence,
        ))
    });
    values.dedup_by(|left, right| {
        left.min_amount == right.min_amount
            && left.max_amount == right.max_amount
            && left.unit == right.unit
    });
}

pub(crate) fn approximate_durations(values: &mut Vec<TemporalApproximateDuration>) {
    values.sort_by(|left, right| {
        (left.amount, left.approximation, left.unit, &left.evidence).cmp(&(
            right.amount,
            right.approximation,
            right.unit,
            &right.evidence,
        ))
    });
    values.dedup_by(|left, right| {
        left.amount == right.amount
            && left.approximation == right.approximation
            && left.unit == right.unit
    });
}

pub(crate) fn event_relations(values: &mut Vec<TemporalEventRelation>) {
    values.sort_by(|left, right| {
        (
            left.amount,
            left.unit,
            left.direction,
            &left.reference_event,
            &left.evidence,
        )
            .cmp(&(
                right.amount,
                right.unit,
                right.direction,
                &right.reference_event,
                &right.evidence,
            ))
    });
    values.dedup_by(|left, right| {
        left.amount == right.amount
            && left.unit == right.unit
            && left.direction == right.direction
            && left.reference_event == right.reference_event
    });
}

pub(crate) fn intervals(values: &mut Vec<TemporalInterval>) {
    values.sort_by(|left, right| {
        (
            left.start_ns,
            left.end_ns,
            left.start_granularity,
            left.end_granularity,
            left.origin,
            &left.evidence,
        )
            .cmp(&(
                right.start_ns,
                right.end_ns,
                right.start_granularity,
                right.end_granularity,
                right.origin,
                &right.evidence,
            ))
    });
    values.dedup_by(|left, right| {
        left.start_ns == right.start_ns
            && left.end_ns == right.end_ns
            && left.start_granularity == right.start_granularity
            && left.end_granularity == right.end_granularity
            && left.origin == right.origin
    });
}

pub(crate) fn patterns(values: &mut Vec<TemporalPattern>) {
    values.sort_by(|left, right| {
        (
            left.frequency,
            left.interval,
            left.occurrences_per_period,
            left.weekday,
            left.month_day,
            left.month,
            &left.evidence,
        )
            .cmp(&(
                right.frequency,
                right.interval,
                right.occurrences_per_period,
                right.weekday,
                right.month_day,
                right.month,
                &right.evidence,
            ))
    });
    values.dedup_by(|left, right| {
        left.frequency == right.frequency
            && left.interval == right.interval
            && left.occurrences_per_period == right.occurrences_per_period
            && left.weekday == right.weekday
            && left.month_day == right.month_day
            && left.month == right.month
    });
}
