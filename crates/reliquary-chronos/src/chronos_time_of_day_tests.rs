use crate::chronos::assess;
use crate::{TemporalClockPrecision, TemporalResolutionStatus};

#[test]
fn meridiem_clock_time_is_resolved_without_inventing_a_date() {
    let assessment = assess("Call at 6:45 AM.", Some(1_800_000_000_000_000_000));

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.analysis.anchors.is_empty());
    assert_eq!(assessment.analysis.times_of_day.len(), 1);
    let value = &assessment.analysis.times_of_day[0];
    assert_eq!((value.hour, value.minute, value.second), (6, 45, 0));
    assert_eq!(value.precision, TemporalClockPrecision::Minute);
    assert_eq!(value.evidence, "6:45 AM");
}

#[test]
fn meridiem_clock_time_normalizes_midnight_noon_and_seconds() {
    let assessment = assess(
        "Starts 12:00 AM, pauses 12:00 PM, resumes 6:45:30 pm.",
        None,
    );

    let values = &assessment.analysis.times_of_day;
    assert_eq!(values.len(), 3);
    assert_eq!(
        (values[0].hour, values[0].minute, values[0].second),
        (0, 0, 0)
    );
    assert_eq!(
        (values[1].hour, values[1].minute, values[1].second),
        (12, 0, 0)
    );
    assert_eq!(
        (values[2].hour, values[2].minute, values[2].second),
        (18, 45, 30)
    );
    assert_eq!(values[2].precision, TemporalClockPrecision::Second);
}

#[test]
fn bare_colon_number_stays_unresolved_until_it_is_calibrated_as_clock_syntax() {
    let assessment = assess("Value 12:34", None);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(assessment.analysis.times_of_day.is_empty());
}

#[test]
fn invalid_meridiem_clock_time_stays_unresolved() {
    let assessment = assess("Call at 13:75 PM.", None);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(assessment.analysis.times_of_day.is_empty());
}
