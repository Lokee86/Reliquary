#![allow(dead_code)]
pub use chronos_detection_model::*;
pub use chronos_inference_model::*;
pub use chronos_model::*;
pub use chronos_resolution_model::*;
pub const CHRONOS_INFERENCE_CONTRACT_VERSION: &str = "chronos-temporal-inference-v1";
#[path = "chronos.rs"]
pub mod chronos;
mod chronos_absolute;
mod chronos_absolute_calendar;
mod chronos_boundary;
mod chronos_boundary_endpoint;
#[cfg(test)]
mod chronos_boundary_tests;
mod chronos_calendar;
#[cfg(test)]
mod chronos_calendar_language_tests;
pub mod chronos_choices;
mod chronos_detection;
mod chronos_detection_context;
mod chronos_detection_explicit;
mod chronos_detection_model;
#[cfg(test)]
mod chronos_detection_tests;
mod chronos_duration;
#[cfg(test)]
mod chronos_duration_recurrence_tests;
#[cfg(test)]
mod chronos_duration_uncertainty_tests;
mod chronos_edit_distance;
mod chronos_event_relative;
#[cfg(test)]
mod chronos_event_relative_tests;
mod chronos_fuzzy;
mod chronos_inference_apply;
mod chronos_inference_model;
pub mod chronos_inference_verify;
mod chronos_loose_calendar;
mod chronos_loose_explicit;
mod chronos_loose_relative;
mod chronos_match;
mod chronos_model;
mod chronos_normalize;
mod chronos_number;
mod chronos_parser;
pub mod chronos_processing_epoch;
#[cfg(test)]
mod chronos_processing_epoch_tests;
mod chronos_recurrence;
mod chronos_recurrence_interval;
mod chronos_relative;
mod chronos_relative_offset;
#[cfg(test)]
mod chronos_relative_tests;
mod chronos_resolution;
mod chronos_resolution_model;
#[cfg(test)]
mod chronos_resolution_tests;
mod chronos_season;
#[cfg(test)]
mod chronos_tests;
mod chronos_time_of_day;
#[cfg(test)]
mod chronos_time_of_day_tests;
mod chronos_vocabulary;

#[derive(Debug)]
pub enum TemporalInferenceError {
    InvalidOutput(String),
}
impl std::fmt::Display for TemporalInferenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOutput(message) => write!(f, "invalid Chronos inference: {message}"),
        }
    }
}
impl std::error::Error for TemporalInferenceError {}
