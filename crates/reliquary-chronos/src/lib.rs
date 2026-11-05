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

mod chronos_analysis;
mod chronos_assessment;
mod chronos_inference_apply;
mod chronos_inference_model;
mod chronos_inference_verify;
mod chronos_merge;
mod chronos_resolution;
mod chronos_resolution_model;
pub use chronos_analysis::{analyze, analyze_with_inference, assess, detect, temporal_matches};
pub use chronos_inference_model::*;
pub use chronos_resolution_model::*;
pub const CHRONOS_INFERENCE_CONTRACT_VERSION: &str = "chronos-temporal-inference-v1";
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporalInferenceError {
    InvalidOutput(String),
}
impl std::fmt::Display for TemporalInferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOutput(reason) => f.write_str(reason),
        }
    }
}
impl std::error::Error for TemporalInferenceError {}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "chronos_detection_tests.rs"]
mod detection_tests;

#[cfg(test)]
mod chronos_inference_tests;
