use crate::chronos::{analyze, detect};
use time::{Date, Month};

#[test]
fn numeric_and_word_number_month_year_offsets_resolve_from_source_time() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze(
        "three months ago, in 2 years, and twenty-one weeks ago",
        Some(reference),
    );

    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::May, 24)
            && anchor.evidence == "three months ago"
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2028, Month::August, 24) && anchor.evidence == "in 2 years"
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::March, 30)
            && anchor.evidence == "twenty-one weeks ago"
    }));
}

#[test]
fn existing_numeric_day_week_offsets_remain_compatible() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze("in 10 days and 2 weeks ago", Some(reference));

    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::September, 3)
            && anchor.evidence == "in 10 days"
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::August, 10) && anchor.evidence == "2 weeks ago"
    }));
}

#[test]
fn month_and_year_offsets_clamp_end_of_month_and_leap_day() {
    let january_31 = timestamp_ns(2024, Month::January, 31, 12);
    let leap_day = timestamp_ns(2024, Month::February, 29, 12);
    let march_31 = timestamp_ns(2024, Month::March, 31, 12);

    let next_month = analyze("in one month", Some(january_31));
    assert_eq!(
        next_month.anchors[0].start_ns,
        day_start_ns(2024, Month::February, 29)
    );

    let previous_month = analyze("one month ago", Some(march_31));
    assert_eq!(
        previous_month.anchors[0].start_ns,
        day_start_ns(2024, Month::February, 29)
    );

    let next_year = analyze("in one year", Some(leap_day));
    assert_eq!(
        next_year.anchors[0].start_ns,
        day_start_ns(2025, Month::February, 28)
    );
}

#[test]
fn relative_offsets_require_authoritative_reference_time() {
    let detection = detect("three months ago");
    assert!(detection.has_indications());
    assert!(analyze("three months ago", None).anchors.is_empty());
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
