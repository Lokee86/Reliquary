use crate::chronos::{analyze, detect, temporal_matches};
use crate::{TemporalGranularity, TemporalIndicationKind, TemporalMatchKind, TemporalOrigin};
use time::{Date, Month};

#[test]
fn hemisphere_qualified_seasons_resolve_to_meteorological_spans() {
    let analysis = analyze(
        "Northern spring 2027 and southern hemisphere summer 2027.",
        None,
    );

    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Season
            && anchor.start_ns == day_start_ns(2027, Month::March, 1)
            && anchor.end_ns == day_start_ns(2027, Month::June, 1)
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Season
            && anchor.start_ns == day_start_ns(2027, Month::December, 1)
            && anchor.end_ns == day_start_ns(2028, Month::March, 1)
    }));
}

#[test]
fn qualified_seasons_participate_in_exact_temporal_matching() {
    let left = analyze("northern autumn 2027", None);
    let right = analyze("fall 2027 in the northern hemisphere", None);
    let (matches, score) = temporal_matches(&left, &right);

    assert!(score > 0.8);
    assert!(
        matches
            .iter()
            .any(|matched| matched.kind == TemporalMatchKind::ExactSeason)
    );
}

#[test]
fn unqualified_season_is_detected_but_not_assigned_a_hemisphere() {
    let text = "The work starts in spring 2027.";
    let detection = detect(text);
    assert!(detection.indications.iter().any(|indication| {
        indication.kind == TemporalIndicationKind::Calendar
            && indication.evidence.eq_ignore_ascii_case("spring")
    }));
    assert!(
        !analyze(text, None)
            .anchors
            .iter()
            .any(|anchor| anchor.granularity == TemporalGranularity::Season)
    );
}

#[test]
fn safe_explicit_loose_calendar_forms_resolve_without_reference_time() {
    let analysis = analyze(
        "Ship Sep. 7th, 2027; renew 7th of May 2028; review Nov 2029.",
        None,
    );

    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.start_ns == day_start_ns(2027, Month::September, 7)
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.start_ns == day_start_ns(2028, Month::May, 7)
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Month
            && anchor.start_ns == day_start_ns(2029, Month::November, 1)
    }));
}

#[test]
fn named_relative_month_uses_the_reference_calendar() {
    let reference = timestamp_ns(2026, Month::August, 24);
    let analysis = analyze(
        "Schedule next March and compare last March with this September.",
        Some(reference),
    );

    assert!(relative_month(&analysis, 2027, Month::March, "next March"));
    assert!(relative_month(&analysis, 2026, Month::March, "last March"));
    assert!(relative_month(
        &analysis,
        2026,
        Month::September,
        "this September"
    ));
}

#[test]
fn explicit_relative_year_date_precedes_generic_year_period() {
    let reference = timestamp_ns(2026, Month::August, 24);
    let analysis = analyze("Deadline March 7 next year.", Some(reference));

    let anchor = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.evidence.eq_ignore_ascii_case("March 7 next year"))
        .expect("specific loose date should resolve as one anchor");
    assert_eq!(anchor.granularity, TemporalGranularity::Day);
    assert_eq!(anchor.origin, TemporalOrigin::Relative);
    assert_eq!(anchor.start_ns, day_start_ns(2027, Month::March, 7));
    assert!(!analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Year
            && anchor.evidence.eq_ignore_ascii_case("next year")
    }));
}

#[test]
fn reference_bound_loose_calendar_forms_remain_unresolved_without_reference_time() {
    let analysis = analyze("Schedule next March and March 7 next year.", None);
    assert!(analysis.anchors.is_empty());
}

fn relative_month(
    analysis: &crate::TemporalAnalysis,
    year: i32,
    month: Month,
    evidence: &str,
) -> bool {
    analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Month
            && anchor.origin == TemporalOrigin::Relative
            && anchor.start_ns == day_start_ns(year, month, 1)
            && anchor.evidence.eq_ignore_ascii_case(evidence)
    })
}

fn timestamp_ns(year: i32, month: Month, day: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    i64::try_from(
        date.with_hms(12, 0, 0)
            .unwrap()
            .assume_utc()
            .unix_timestamp_nanos(),
    )
    .unwrap()
}

fn day_start_ns(year: i32, month: Month, day: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    i64::try_from(date.midnight().assume_utc().unix_timestamp_nanos()).unwrap()
}
