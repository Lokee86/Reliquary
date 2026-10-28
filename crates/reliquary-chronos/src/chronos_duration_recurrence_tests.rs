use crate::chronos::{analyze, detect};
use crate::{TemporalDurationUnit, TemporalFrequency, TemporalIndicationKind, TemporalWeekday};

#[test]
fn standalone_durations_resolve_without_reference_time() {
    let analysis = analyze(
        "The outage lasted three hours; repairs took 2 weeks; warranty duration of one year.",
        None,
    );

    assert!(analysis.durations.iter().any(|duration| {
        duration.amount == 3
            && duration.unit == TemporalDurationUnit::Hour
            && duration.evidence == "lasted three hours"
    }));
    assert!(analysis.durations.iter().any(|duration| {
        duration.amount == 2
            && duration.unit == TemporalDurationUnit::Week
            && duration.evidence == "took 2 weeks"
    }));
    assert!(analysis.durations.iter().any(|duration| {
        duration.amount == 1
            && duration.unit == TemporalDurationUnit::Year
            && duration.evidence == "duration of one year"
    }));
}

#[test]
fn relative_offsets_are_not_misclassified_as_standalone_durations() {
    let analysis = analyze("three months ago and in two weeks", None);
    assert!(analysis.durations.is_empty());
}

#[test]
fn normalized_duration_evidence_maps_back_to_original_text() {
    let analysis = analyze("The build lasted three mnoths.", None);
    let duration = analysis
        .durations
        .iter()
        .find(|duration| duration.unit == TemporalDurationUnit::Month)
        .expect("corrected duration unit should resolve");

    assert_eq!(duration.amount, 3);
    assert_eq!(duration.evidence, "lasted three mnoths");
}

#[test]
fn recurrence_supports_word_numbers_and_unambiguous_alternation() {
    let analysis = analyze(
        "Back up every three weeks, inspect every other Tuesday, rotate every other month.",
        None,
    );

    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Weekly
            && pattern.interval == 3
            && pattern.weekday.is_none()
            && pattern.evidence == "every three weeks"
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Weekly
            && pattern.interval == 2
            && pattern.weekday == Some(TemporalWeekday::Tuesday)
            && pattern.evidence == "every other Tuesday"
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Monthly
            && pattern.interval == 2
            && pattern.weekday.is_none()
            && pattern.evidence == "every other month"
    }));
}

#[test]
fn specific_calendar_recurrence_is_not_broadened_by_generic_recurrence() {
    let analysis = analyze("every month on the 5th; every year on March 7", None);

    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Monthly && pattern.month_day == Some(5)
    }));
    assert!(!analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Monthly
            && pattern.month_day.is_none()
            && pattern.evidence == "every month"
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Yearly
            && pattern.month == Some(3)
            && pattern.month_day == Some(7)
    }));
    assert!(!analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Yearly
            && pattern.month.is_none()
            && pattern.month_day.is_none()
            && pattern.evidence == "every year"
    }));
}

#[test]
fn ambiguous_cadence_is_detected_but_not_guessed() {
    let detection = detect("A biweekly review is required.");
    assert!(detection.indications.iter().any(|indication| {
        indication.kind == TemporalIndicationKind::Recurrence
            && indication.evidence.eq_ignore_ascii_case("biweekly")
    }));
    assert!(
        analyze("A biweekly review is required.", None)
            .patterns
            .is_empty()
    );
}

#[test]
fn existing_numeric_and_single_unit_recurrence_remain_compatible() {
    let analysis = analyze("every 2 weeks; each month; Fridays", None);

    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Weekly && pattern.interval == 2
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Monthly && pattern.interval == 1
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == TemporalFrequency::Weekly
            && pattern.interval == 1
            && pattern.weekday == Some(TemporalWeekday::Friday)
    }));
}
