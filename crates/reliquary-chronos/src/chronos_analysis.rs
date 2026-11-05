use crate::{
    TemporalAnalysis, TemporalAssessment, TemporalDetection, TemporalInference,
    TemporalInferenceError, TemporalMatch,
};

/// Detect possible world-time material without requiring that Chronos can already parse it.
pub fn detect(text: &str) -> TemporalDetection {
    crate::chronos_detection::detect_and_normalize(text).detection
}

/// Deterministically analyze temporal language against an optional authoritative reference time.
///
/// Chronos does not choose the reference chronology. Consumers such as Dream and Insomnia resolve
/// their authoritative source/reference time and pass it through this shared boundary.
pub fn analyze(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalAnalysis {
    let detected = crate::chronos_detection::detect_and_normalize(text);
    analyze_detected(text, reference_timestamp_ns, &detected)
}

/// Detect and deterministically analyze temporal material, then report whether any detected
/// temporal evidence remains unresolved and is therefore eligible for bounded inference.
pub fn assess(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalAssessment {
    let detected = crate::chronos_detection::detect_and_normalize(text);
    let analysis = analyze_detected(text, reference_timestamp_ns, &detected);
    let detection = detected.detection.clone();
    let resolution = crate::chronos_assessment::assess_resolution(text, &detection, &analysis);
    TemporalAssessment {
        detection,
        analysis,
        resolution,
    }
}

/// Rehydrate a verified bounded inference by deterministically re-parsing its canonical
/// expressions and merging them into ordinary derived Chronos analysis.
pub fn analyze_with_inference(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    inference: &TemporalInference,
) -> Result<TemporalAnalysis, TemporalInferenceError> {
    crate::chronos_inference_apply::apply_inference(
        text,
        analyze(text, reference_timestamp_ns),
        inference,
        reference_timestamp_ns,
    )
}

fn analyze_detected(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    detected: &crate::chronos_detection::DetectedTemporalText,
) -> TemporalAnalysis {
    // A blocked local definition is not an asserted recurrence, even if the
    // general parser recognizes words inside it. Dream consumes analyze directly,
    // so resolution status alone cannot keep contradictory claims out.
    let masked = crate::chronos_resolution::mask_blocked_definition_clauses(
        text,
        reference_timestamp_ns,
        &detected.detection,
    );
    let masked_detected = masked
        .as_deref()
        .map(crate::chronos_detection::detect_and_normalize);
    let parse_text = masked.as_deref().unwrap_or(text);
    let parse_detected = masked_detected.as_ref().unwrap_or(detected);
    let mut analysis = crate::chronos_parser::parse_temporal(parse_text, reference_timestamp_ns);
    let Some(normalized_text) = parse_detected.normalized_text.as_deref() else {
        crate::chronos_resolution::apply_explicit_definitions(
            text,
            reference_timestamp_ns,
            &detected.detection,
            &mut analysis,
        );
        return analysis;
    };

    let mut corrected =
        crate::chronos_parser::parse_temporal(normalized_text, reference_timestamp_ns);
    for anchor in &mut corrected.anchors {
        anchor.evidence = parse_detected.restore_evidence(parse_text, &anchor.evidence);
    }
    for time_of_day in &mut corrected.times_of_day {
        time_of_day.evidence = parse_detected.restore_evidence(parse_text, &time_of_day.evidence);
    }
    for duration in &mut corrected.durations {
        duration.evidence = parse_detected.restore_evidence(parse_text, &duration.evidence);
    }
    for duration in &mut corrected.duration_ranges {
        duration.evidence = parse_detected.restore_evidence(parse_text, &duration.evidence);
    }
    for duration in &mut corrected.approximate_durations {
        duration.evidence = parse_detected.restore_evidence(parse_text, &duration.evidence);
    }
    for relation in &mut corrected.event_relations {
        relation.evidence = parse_detected.restore_evidence(parse_text, &relation.evidence);
    }
    for interval in &mut corrected.intervals {
        interval.evidence = parse_detected.restore_evidence(parse_text, &interval.evidence);
    }
    for pattern in &mut corrected.patterns {
        pattern.evidence = parse_detected.restore_evidence(parse_text, &pattern.evidence);
    }
    crate::chronos_parser::merge_temporal_analysis(&mut analysis, corrected);
    crate::chronos_resolution::apply_explicit_definitions(
        text,
        reference_timestamp_ns,
        &detected.detection,
        &mut analysis,
    );
    analysis
}

pub fn temporal_matches(
    source: &TemporalAnalysis,
    candidate: &TemporalAnalysis,
) -> (Vec<TemporalMatch>, f64) {
    crate::chronos_match::temporal_matches(source, candidate)
}
