use crate::chronos::{analyze, assess};
use crate::{TemporalDurationUnit, TemporalEventDirection, TemporalResolutionStatus};

#[test]
fn exact_offset_before_event_resolves_without_event_timestamp() {
    let assessment = assess("Start 30 minutes before cooking.", None);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert_eq!(assessment.analysis.event_relations.len(), 1);
    let relation = &assessment.analysis.event_relations[0];
    assert_eq!(relation.amount, 30);
    assert_eq!(relation.unit, TemporalDurationUnit::Minute);
    assert_eq!(relation.direction, TemporalEventDirection::Before);
    assert_eq!(relation.reference_event, "cooking");
    assert_eq!(relation.evidence, "30 minutes before cooking");
    assert!(assessment.analysis.anchors.is_empty());
    assert!(assessment.analysis.intervals.is_empty());
}

#[test]
fn exact_offset_after_noun_phrase_preserves_event_referent() {
    let analysis = analyze(
        "Review two days after the production deployment on Tuesday.",
        None,
    );

    let relation = analysis
        .event_relations
        .iter()
        .find(|relation| relation.direction == TemporalEventDirection::After)
        .expect("event-relative relation should resolve");
    assert_eq!(relation.amount, 2);
    assert_eq!(relation.unit, TemporalDurationUnit::Day);
    assert_eq!(relation.reference_event, "the production deployment");
    assert_eq!(
        relation.evidence,
        "two days after the production deployment"
    );
}

#[test]
fn calendar_endpoint_remains_boundary_semantics_not_event_semantics() {
    let analysis = analyze("Finish 30 minutes before Tuesday.", None);

    assert!(analysis.event_relations.is_empty());
}

#[test]
fn offset_is_required_for_event_relative_semantics() {
    let analysis = analyze("Finish before cooking.", None);

    assert!(analysis.event_relations.is_empty());
}

#[test]
fn normalized_event_relative_evidence_maps_back_to_original_text() {
    let assessment = assess("Start 30 mnutes before cooking.", None);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    let relation = assessment
        .analysis
        .event_relations
        .first()
        .expect("corrected event-relative relation should resolve");
    assert_eq!(relation.amount, 30);
    assert_eq!(relation.unit, TemporalDurationUnit::Minute);
    assert_eq!(relation.reference_event, "cooking");
    assert_eq!(relation.evidence, "30 mnutes before cooking");
}
