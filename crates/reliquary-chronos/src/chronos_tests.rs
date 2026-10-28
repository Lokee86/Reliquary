use crate::chronos::{analyze, detect, temporal_matches};
use crate::{
    TemporalFrequency, TemporalGranularity, TemporalIndicationKind, TemporalMatchKind,
    TemporalOrigin,
};
use time::{Date, Month};

#[test]
fn shared_chronos_api_preserves_deterministic_absolute_relative_and_recurrence_behavior() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze(
        "Deadline 2026-09-15. Review tomorrow. Crew reports every Monday.",
        Some(reference),
    );

    assert_eq!(analysis.source_timestamp_ns, Some(reference));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.origin == TemporalOrigin::Explicit
            && anchor.start_ns == day_start_ns(2026, Month::September, 15)
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Day
            && anchor.origin == TemporalOrigin::Relative
            && anchor.start_ns == day_start_ns(2026, Month::August, 25)
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Weekly && pattern.interval == 1
    }));
}

#[test]
fn unicode_dash_date_ranges_remain_supported() {
    let analysis = analyze("Window 2026-10-01 – 2026-10-03.", None);
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == TemporalGranularity::Range
            && anchor.start_ns == day_start_ns(2026, Month::October, 1)
            && anchor.end_ns == day_start_ns(2026, Month::October, 4)
    }));
}

#[test]
fn shared_chronos_matcher_preserves_exact_day_scoring() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let relative = analyze("Inspection is tomorrow.", Some(reference));
    let explicit = analyze("Inspection is 2026-08-25.", None);

    let (matches, score) = temporal_matches(&relative, &explicit);
    assert!(score > 0.9);
    assert!(
        matches
            .iter()
            .any(|matched| matched.kind == TemporalMatchKind::ExactDay)
    );
}

#[test]
fn detector_preserves_correctable_temporal_material_and_original_span() {
    let text = "That happened three mnoths ago.";
    let detection = detect(text);
    let corrected = detection
        .indications
        .iter()
        .find(|indication| indication.evidence == "mnoths")
        .expect("misspelled temporal unit should be detected");

    assert_eq!(corrected.kind, TemporalIndicationKind::Duration);
    assert_eq!(corrected.normalized.as_deref(), Some("months"));
    assert_eq!(
        &text[corrected.start_byte..corrected.end_byte],
        corrected.evidence
    );
    assert!(detection.has_indications());
    assert!(detection.has_corrections());

    let analysis = analyze(text, Some(timestamp_ns(2026, Month::August, 24, 12)));
    let anchor = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.origin == TemporalOrigin::Relative)
        .expect("normalized word-number month offset should now resolve");
    assert_eq!(anchor.start_ns, day_start_ns(2026, Month::May, 24));
    assert_eq!(anchor.evidence, "three mnoths ago");
}

#[test]
fn bounded_normalization_feeds_existing_parser_and_restores_original_evidence() {
    let reference = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = analyze("Meet next Wendesday.", Some(reference));
    let anchor = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.origin == TemporalOrigin::Relative)
        .expect("corrected weekday should resolve through existing parser");

    assert_eq!(anchor.start_ns, day_start_ns(2026, Month::August, 26));
    assert_eq!(anchor.evidence, "next Wendesday");
}

#[test]
fn bounded_normalization_corrects_calendar_typo_without_rewriting_evidence() {
    let analysis = analyze("Target is Febuary 2027.", None);
    let anchor = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.granularity == TemporalGranularity::Month)
        .expect("corrected month should resolve through existing parser");

    assert_eq!(anchor.start_ns, day_start_ns(2027, Month::February, 1));
    assert_eq!(anchor.evidence, "Febuary 2027");
}

#[test]
fn fuzzy_units_require_temporal_context() {
    let detection = detect("The sculpture has several mouths carved into it.");
    assert!(!detection.has_indications());
    assert!(!detection.has_corrections());
}

#[test]
fn detector_covers_existing_parser_years_outside_the_modern_window() {
    let detection = detect("During 1800 the archive changed.");
    assert!(detection.indications.iter().any(|indication| {
        indication.kind == TemporalIndicationKind::Explicit && indication.evidence == "1800"
    }));
    assert!(
        analyze("During 1800 the archive changed.", None)
            .anchors
            .iter()
            .any(|anchor| anchor.granularity == TemporalGranularity::Year)
    );
}

#[test]
fn corrected_evidence_maps_to_the_exact_original_occurrence() {
    let analysis = analyze("february 2026 and Febuary 2027", None);
    let first = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.start_ns == day_start_ns(2026, Month::February, 1))
        .unwrap();
    let second = analysis
        .anchors
        .iter()
        .find(|anchor| anchor.start_ns == day_start_ns(2027, Month::February, 1))
        .unwrap();

    assert_eq!(first.evidence, "february 2026");
    assert_eq!(second.evidence, "Febuary 2027");
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
