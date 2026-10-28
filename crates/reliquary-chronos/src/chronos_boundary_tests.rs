use crate::chronos::analyze;
use crate::{TemporalGranularity, TemporalOrigin};
use time::{Date, Month};

#[test]
fn open_boundaries_use_explicit_optional_interval_bounds() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze(
        "since May; until Friday; after 2026-10-01; before 2026-12-01",
        Some(reference),
    );

    let since = interval(&analysis, "since May");
    assert_eq!(since.start_ns, Some(day_start_ns(2026, Month::May, 1)));
    assert_eq!(since.end_ns, None);
    assert_eq!(since.start_granularity, Some(TemporalGranularity::Month));

    let until = interval(&analysis, "until Friday");
    assert_eq!(until.start_ns, None);
    assert_eq!(until.end_ns, Some(day_start_ns(2026, Month::August, 29)));
    assert_eq!(until.end_granularity, Some(TemporalGranularity::Day));

    let after = interval(&analysis, "after 2026-10-01");
    assert_eq!(after.start_ns, Some(day_start_ns(2026, Month::October, 2)));
    assert_eq!(after.end_ns, None);
    assert_eq!(after.origin, TemporalOrigin::Explicit);

    let before = interval(&analysis, "before 2026-12-01");
    assert_eq!(before.start_ns, None);
    assert_eq!(before.end_ns, Some(day_start_ns(2026, Month::December, 1)));
}

#[test]
fn starting_and_ending_accept_existing_relative_periods() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze("starting next week; ending next week", Some(reference));

    let starting = interval(&analysis, "starting next week");
    assert_eq!(
        starting.start_ns,
        Some(day_start_ns(2026, Month::August, 31))
    );
    assert_eq!(starting.end_ns, None);

    let ending = interval(&analysis, "ending next week");
    assert_eq!(ending.start_ns, None);
    assert_eq!(ending.end_ns, Some(day_start_ns(2026, Month::September, 7)));
}

#[test]
fn from_month_ranges_use_reference_year_and_cross_year_when_needed() {
    let reference = timestamp_ns(2026, Month::September, 15, 12);
    let analysis = analyze(
        "from June to August and from November to February",
        Some(reference),
    );

    let summer = interval(&analysis, "from June to August");
    assert_eq!(summer.start_ns, Some(day_start_ns(2026, Month::June, 1)));
    assert_eq!(summer.end_ns, Some(day_start_ns(2026, Month::September, 1)));

    let winter = interval(&analysis, "from November to February");
    assert_eq!(
        winter.start_ns,
        Some(day_start_ns(2026, Month::November, 1))
    );
    assert_eq!(winter.end_ns, Some(day_start_ns(2027, Month::March, 1)));

    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Range
            && anchor.evidence == "from November to February"
    }));
}

#[test]
fn existing_iso_ranges_are_mirrored_into_intervals_without_changing_anchor_behavior() {
    let analysis = analyze("Window 2026-10-01 to 2026-10-03.", None);
    let range_interval = interval(&analysis, "2026-10-01 to 2026-10-03");

    assert_eq!(
        range_interval.start_ns,
        Some(day_start_ns(2026, Month::October, 1))
    );
    assert_eq!(
        range_interval.end_ns,
        Some(day_start_ns(2026, Month::October, 4))
    );
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Range
            && anchor.start_ns == day_start_ns(2026, Month::October, 1)
            && anchor.end_ns == day_start_ns(2026, Month::October, 4)
    }));

    let until = analyze("Window 2026-10-01 until 2026-10-03.", None);
    let until_interval = interval(&until, "2026-10-01 until 2026-10-03");
    assert_eq!(until_interval.start_ns, range_interval.start_ns);
    assert_eq!(until_interval.end_ns, range_interval.end_ns);
}

#[test]
fn loose_boundary_calendar_terms_require_reference_time_but_explicit_dates_do_not() {
    assert!(analyze("since May", None).intervals.is_empty());
    let explicit = analyze("since 2026-05-01", None);
    assert_eq!(
        interval(&explicit, "since 2026-05-01").start_ns,
        Some(day_start_ns(2026, Month::May, 1))
    );
}

#[test]
fn normalized_boundary_evidence_maps_back_to_original_text() {
    let analysis = analyze("since Febuary 2027", None);
    let interval = interval(&analysis, "since Febuary 2027");
    assert_eq!(
        interval.start_ns,
        Some(day_start_ns(2027, Month::February, 1))
    );
    assert_eq!(interval.evidence, "since Febuary 2027");
}

fn interval<'a>(
    analysis: &'a crate::TemporalAnalysis,
    evidence: &str,
) -> &'a crate::TemporalInterval {
    analysis
        .intervals
        .iter()
        .find(|interval| interval.evidence == evidence)
        .unwrap_or_else(|| panic!("missing interval {evidence:?}: {:#?}", analysis.intervals))
}

fn timestamp_ns(year: i32, month: Month, day: u8, hour: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    let value = date.with_hms(hour, 0, 0).unwrap().assume_utc();
    i64::try_from(value.unix_timestamp_nanos()).unwrap()
}

fn day_start_ns(year: i32, month: Month, day: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    i64::try_from(date.midnight().assume_utc().unix_timestamp_nanos()).unwrap()
}
