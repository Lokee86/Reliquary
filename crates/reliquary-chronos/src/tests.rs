use super::{
    TemporalDurationUnit, TemporalFrequency, TemporalGranularity, TemporalMatchKind,
    TemporalOrigin, analyze, temporal_matches,
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

#[test]
fn temporal_match_scores_exact_overlap_and_unrelated_dates() {
    let first = analyze("The launch was on 2024-05-13.", None);
    let same = analyze("The release was on 2024-05-13.", None);
    let different = analyze("The release was on 2024-05-14.", None);
    let (matches, score) = temporal_matches(&first, &same);
    assert_eq!(score, 0.98);
    assert_eq!(matches[0].kind, TemporalMatchKind::ExactDay);
    assert_eq!(temporal_matches(&first, &different), (vec![], 0.0));
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

#[test]
fn retained_calendar_edges_handle_hemispheres_and_leap_day_clamping() {
    let seasons = analyze(
        "Northern spring 2027 and southern hemisphere summer 2027.",
        None,
    );
    for (start, end) in [
        ((2027, Month::March, 1), (2027, Month::June, 1)),
        ((2027, Month::December, 1), (2028, Month::March, 1)),
    ] {
        assert!(seasons.anchors.iter().any(|anchor| {
            anchor.granularity == TemporalGranularity::Season
                && anchor.start_ns == day_start_ns(start.0, start.1, start.2)
                && anchor.end_ns == day_start_ns(end.0, end.1, end.2)
        }));
    }
    assert!(
        !analyze("The work starts in spring 2027.", None)
            .anchors
            .iter()
            .any(|anchor| anchor.granularity == TemporalGranularity::Season)
    );

    for (source, reference, expected) in [
        (
            "in one month",
            (2024, Month::January, 31),
            (2024, Month::February, 29),
        ),
        (
            "one month ago",
            (2024, Month::March, 31),
            (2024, Month::February, 29),
        ),
        (
            "in one year",
            (2024, Month::February, 29),
            (2025, Month::February, 28),
        ),
    ] {
        let reference_ns = timestamp_ns(reference.0, reference.1, reference.2, 12);
        let result = analyze(source, Some(reference_ns));
        assert_eq!(
            result.anchors.first().map(|anchor| anchor.start_ns),
            Some(day_start_ns(expected.0, expected.1, expected.2)),
            "{source}"
        );
    }
}

#[test]
fn retained_clock_edges_normalize_meridiem_without_fabricating_a_date() {
    let assessment = super::assess(
        "Starts 12:00 AM, pauses 12:00 PM, resumes 6:45:30 pm.",
        None,
    );
    assert!(assessment.analysis.anchors.is_empty());
    let times = &assessment.analysis.times_of_day;
    assert_eq!(times.len(), 3);
    assert_eq!((times[0].hour, times[0].minute, times[0].second), (0, 0, 0));
    assert_eq!(
        (times[1].hour, times[1].minute, times[1].second),
        (12, 0, 0)
    );
    assert_eq!(
        (times[2].hour, times[2].minute, times[2].second),
        (18, 45, 30)
    );
    assert_eq!(times[2].precision, super::TemporalClockPrecision::Second);

    let invalid = super::assess("Call at 13:75 PM.", None);
    assert!(invalid.analysis.times_of_day.is_empty());
    assert_eq!(
        invalid.resolution.status,
        super::TemporalResolutionStatus::Unresolved
    );
}
