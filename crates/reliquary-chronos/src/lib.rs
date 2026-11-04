//! Deterministic temporal parsing with no owner-storage dependency.

mod chronos_absolute;
mod chronos_absolute_calendar;
mod chronos_boundary;
mod chronos_boundary_endpoint;
mod chronos_calendar;
mod chronos_detection;
mod chronos_detection_context;
mod chronos_detection_explicit;
mod chronos_detection_model;
mod chronos_duration;
mod chronos_edit_distance;
mod chronos_event_relative;
mod chronos_fuzzy;
mod chronos_loose_calendar;
mod chronos_loose_explicit;
mod chronos_loose_relative;
mod chronos_match;
mod chronos_model;
mod chronos_normalize;
mod chronos_number;
mod chronos_parser;
mod chronos_recurrence;
mod chronos_recurrence_interval;
mod chronos_relative;
mod chronos_relative_offset;
mod chronos_season;
mod chronos_time_of_day;
mod chronos_vocabulary;

pub use chronos_detection_model::{TemporalDetection, TemporalIndication, TemporalIndicationKind};
pub use chronos_model::*;

/// Deterministically extracts temporal information from text.
///
/// Relative expressions are resolved only when the caller supplies an
/// authoritative reference timestamp. Chronos does not choose or persist it.
pub fn analyze(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalAnalysis {
    let detected = chronos_detection::detect_and_normalize(text);
    let parse_text = detected.normalized_text.as_deref().unwrap_or(text);
    let mut analysis = chronos_parser::parse_temporal(parse_text, reference_timestamp_ns);
    for anchor in &mut analysis.anchors {
        anchor.evidence = detected.restore_evidence(text, &anchor.evidence);
    }
    for time in &mut analysis.times_of_day {
        time.evidence = detected.restore_evidence(text, &time.evidence);
    }
    for duration in &mut analysis.durations {
        duration.evidence = detected.restore_evidence(text, &duration.evidence);
    }
    for duration in &mut analysis.duration_ranges {
        duration.evidence = detected.restore_evidence(text, &duration.evidence);
    }
    for duration in &mut analysis.approximate_durations {
        duration.evidence = detected.restore_evidence(text, &duration.evidence);
    }
    for relation in &mut analysis.event_relations {
        relation.evidence = detected.restore_evidence(text, &relation.evidence);
    }
    for interval in &mut analysis.intervals {
        interval.evidence = detected.restore_evidence(text, &interval.evidence);
    }
    for pattern in &mut analysis.patterns {
        pattern.evidence = detected.restore_evidence(text, &pattern.evidence);
    }
    analysis
}

/// Identifies potentially temporal text and corrects only tightly bounded likely typos.
pub fn detect(text: &str) -> TemporalDetection {
    chronos_detection::detect_and_normalize(text).detection
}

/// Compare caller-supplied temporal analyses without accessing owner state.
pub fn temporal_matches(
    source: &TemporalAnalysis,
    candidate: &TemporalAnalysis,
) -> (Vec<TemporalMatch>, f64) {
    chronos_match::temporal_matches(source, candidate)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "chronos_detection_tests.rs"]
mod detection_tests;
