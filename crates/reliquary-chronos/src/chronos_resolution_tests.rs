use crate::chronos::assess;
use crate::{TemporalIndicationKind, TemporalResolutionStatus};
use time::{Date, Month};

#[test]
fn no_temporal_material_is_distinct_from_fully_resolved() {
    let assessment = assess("The build passed cleanly.", None);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::NoTemporalMaterial
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
    assert!(!assessment.resolution.needs_inference());
}

#[test]
fn fully_resolved_material_has_no_unresolved_residue() {
    let assessment = assess(
        "Deadline 2026-09-15 and review next Tuesday.",
        Some(reference()),
    );
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
    assert!(!assessment.resolution.needs_inference());
}

#[test]
fn detected_but_ambiguous_material_remains_unresolved() {
    let assessment = assess("A biweekly review starts in spring 2027.", None);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(assessment.resolution.needs_inference());
    assert!(
        assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Recurrence
                    && value.evidence.eq_ignore_ascii_case("biweekly")
            })
    );
    assert!(
        assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Calendar
                    && value.evidence.eq_ignore_ascii_case("spring")
            })
    );
}

#[test]
fn mixed_resolved_and_unresolved_text_reports_only_the_residue() {
    let assessment = assess("Deadline 2026-09-15; review biweekly.", None);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(
        assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Recurrence
                    && value.evidence.eq_ignore_ascii_case("biweekly")
            })
    );
    assert!(
        !assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Explicit && value.evidence == "2026-09-15"
            })
    );
}

#[test]
fn relative_material_without_reference_time_is_unresolved() {
    let assessment = assess("Review tomorrow.", None);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(
        assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Relative
                    && value.evidence.eq_ignore_ascii_case("tomorrow")
            })
    );
}

#[test]
fn normalized_temporal_evidence_counts_as_resolved() {
    let assessment = assess("Target is Febuary 2027.", None);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
}

fn reference() -> i64 {
    let date = Date::from_calendar_date(2026, Month::September, 16).unwrap();
    i64::try_from(
        date.with_hms(12, 0, 0)
            .unwrap()
            .assume_utc()
            .unix_timestamp_nanos(),
    )
    .unwrap()
}
