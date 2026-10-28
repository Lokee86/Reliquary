use crate::chronos::{analyze, assess};
use crate::{TemporalDurationApproximation, TemporalDurationUnit, TemporalResolutionStatus};

#[test]
fn duration_ranges_preserve_numeric_bounds() {
    let analysis = analyze(
        "Repairs took 10-15 minutes and backup lasted three to five hours.",
        None,
    );

    assert!(analysis.duration_ranges.iter().any(|duration| {
        duration.min_amount == 10
            && duration.max_amount == 15
            && duration.unit == TemporalDurationUnit::Minute
            && duration.evidence == "took 10-15 minutes"
    }));
    assert!(analysis.duration_ranges.iter().any(|duration| {
        duration.min_amount == 3
            && duration.max_amount == 5
            && duration.unit == TemporalDurationUnit::Hour
            && duration.evidence == "lasted three to five hours"
    }));
    assert!(analysis.durations.is_empty());
}

#[test]
fn approximate_durations_preserve_uncertainty_without_inventing_bounds() {
    let assessment = assess(
        "The outage lasted about two weeks; repair took a few hours; migration lasted a couple of days; soak test lasted several hours.",
        None,
    );

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    let values = &assessment.analysis.approximate_durations;
    assert!(values.iter().any(|duration| {
        duration.amount == Some(2)
            && duration.approximation == TemporalDurationApproximation::About
            && duration.unit == TemporalDurationUnit::Week
    }));
    assert!(values.iter().any(|duration| {
        duration.amount.is_none()
            && duration.approximation == TemporalDurationApproximation::Few
            && duration.unit == TemporalDurationUnit::Hour
    }));
    assert!(values.iter().any(|duration| {
        duration.amount.is_none()
            && duration.approximation == TemporalDurationApproximation::Couple
            && duration.unit == TemporalDurationUnit::Day
    }));
    assert!(values.iter().any(|duration| {
        duration.amount.is_none()
            && duration.approximation == TemporalDurationApproximation::Several
            && duration.unit == TemporalDurationUnit::Hour
    }));
    assert!(assessment.analysis.durations.is_empty());
}

#[test]
fn normalized_approximate_duration_evidence_maps_back_to_original_text() {
    let analysis = analyze("The build lasted about three mnoths.", None);
    let duration = analysis
        .approximate_durations
        .iter()
        .find(|duration| duration.unit == TemporalDurationUnit::Month)
        .expect("corrected approximate duration should resolve");

    assert_eq!(duration.amount, Some(3));
    assert_eq!(duration.evidence, "lasted about three mnoths");
}
