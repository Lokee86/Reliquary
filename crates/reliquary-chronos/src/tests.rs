use super::{
    TemporalDurationUnit, TemporalFrequency, TemporalGranularity, TemporalOrigin, analyze,
};
use time::{Date, Month, PrimitiveDateTime, Time};

fn timestamp_ns(year: i32, month: Month, day: u8, hour: u8) -> i64 {
    PrimitiveDateTime::new(
        Date::from_calendar_date(year, month, day).unwrap(),
        Time::from_hms(hour, 0, 0).unwrap(),
    )
    .assume_utc()
    .unix_timestamp_nanos() as i64
}

fn day_start_ns(year: i32, month: Month, day: u8) -> i64 {
    timestamp_ns(year, month, day, 0)
}

#[test]
fn parses_iso_date_and_rfc3339_instant() {
    let analysis = analyze("Due 2026-09-15; event at 2026-09-15T10:30:00Z.", None);
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.origin == TemporalOrigin::Explicit
            && anchor.start_ns == day_start_ns(2026, Month::September, 15)
    }));
    let instant_ns = timestamp_ns(2026, Month::September, 15, 10) + 30 * 60 * 1_000_000_000;
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Instant && anchor.start_ns == instant_ns
    }));
}

#[test]
fn resolves_relative_day_against_supplied_reference_time() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze("Review tomorrow.", Some(reference));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.origin == TemporalOrigin::Relative
            && anchor.start_ns == day_start_ns(2026, Month::August, 25)
    }));
}

#[test]
fn parses_range_duration_and_recurrence() {
    let analysis = analyze(
        "Window 2026-10-01 through 2026-10-03; lasting 3 days; review every Monday.",
        None,
    );
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Range
            && anchor.start_ns == day_start_ns(2026, Month::October, 1)
            && anchor.end_ns == day_start_ns(2026, Month::October, 4)
    }));
    assert!(
        analysis
            .durations
            .iter()
            .any(|duration| duration.amount == 3 && duration.unit == TemporalDurationUnit::Day)
    );
    assert!(
        analysis
            .patterns
            .iter()
            .any(|pattern| pattern.frequency == TemporalFrequency::Weekly)
    );
}

#[test]
fn without_reference_time_does_not_invent_relative_anchor() {
    let analysis = analyze("Review tomorrow.", None);
    assert!(analysis.anchors.is_empty());
}

#[test]
fn bounded_typo_normalization_parses_time_but_preserves_original_evidence() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze("It happened three mnoths ago.", Some(reference));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.origin == TemporalOrigin::Relative && anchor.evidence.contains("mnoths")
    }));
}

#[test]
fn detector_returns_original_byte_span_and_normalized_token() {
    let source = "That happened three mnoths ago.";
    let detection = super::detect(source);
    let indication = detection
        .indications
        .iter()
        .find(|value| value.normalized.as_deref() == Some("months"))
        .unwrap();
    assert_eq!(
        &source[indication.start_byte..indication.end_byte],
        "mnoths"
    );
    assert_eq!(indication.evidence, "mnoths");
}
