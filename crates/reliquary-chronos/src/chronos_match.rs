use crate::{
    TemporalAnalysis, TemporalAnchor, TemporalGranularity, TemporalMatch, TemporalMatchKind,
    TemporalPattern,
};

const MAX_TEMPORAL_MATCHES: usize = 6;

pub(crate) fn temporal_matches(
    source: &TemporalAnalysis,
    candidate: &TemporalAnalysis,
) -> (Vec<TemporalMatch>, f64) {
    let mut matches = Vec::new();
    let mut best = 0.0_f64;
    for left in &source.anchors {
        for right in &candidate.anchors {
            if !overlap(left, right) {
                continue;
            }
            let (kind, score) = classify_anchor_match(left, right);
            best = best.max(score);
            matches.push(TemporalMatch {
                kind,
                source_anchor: Some(left.clone()),
                candidate_anchor: Some(right.clone()),
                source_pattern: None,
                candidate_pattern: None,
            });
            if matches.len() == MAX_TEMPORAL_MATCHES {
                return (matches, best);
            }
        }
    }
    for left in &source.patterns {
        for right in &candidate.patterns {
            if !same_pattern(left, right) {
                continue;
            }
            best = best.max(0.7);
            matches.push(TemporalMatch {
                kind: TemporalMatchKind::Recurrence,
                source_anchor: None,
                candidate_anchor: None,
                source_pattern: Some(left.clone()),
                candidate_pattern: Some(right.clone()),
            });
            if matches.len() == MAX_TEMPORAL_MATCHES {
                return (matches, best);
            }
        }
    }
    (matches, best)
}

fn overlap(left: &TemporalAnchor, right: &TemporalAnchor) -> bool {
    left.start_ns < right.end_ns && right.start_ns < left.end_ns
}

fn classify_anchor_match(
    left: &TemporalAnchor,
    right: &TemporalAnchor,
) -> (TemporalMatchKind, f64) {
    if left.start_ns == right.start_ns
        && left.end_ns == right.end_ns
        && left.granularity == right.granularity
    {
        return match left.granularity {
            TemporalGranularity::Instant => (TemporalMatchKind::ExactInstant, 1.0),
            TemporalGranularity::Day => (TemporalMatchKind::ExactDay, 0.98),
            TemporalGranularity::Week => (TemporalMatchKind::ExactWeek, 0.92),
            TemporalGranularity::Month => (TemporalMatchKind::ExactMonth, 0.88),
            TemporalGranularity::Quarter => (TemporalMatchKind::ExactQuarter, 0.84),
            TemporalGranularity::Season => (TemporalMatchKind::ExactSeason, 0.82),
            TemporalGranularity::Year => (TemporalMatchKind::ExactYear, 0.8),
            TemporalGranularity::Range => (TemporalMatchKind::RangeOverlap, 0.96),
        };
    }
    (TemporalMatchKind::RangeOverlap, 0.75)
}

fn same_pattern(left: &TemporalPattern, right: &TemporalPattern) -> bool {
    left.frequency == right.frequency
        && left.interval == right.interval
        && left.occurrences_per_period == right.occurrences_per_period
        && left.weekday == right.weekday
        && left.month_day == right.month_day
        && left.month == right.month
}
