use crate::{
    TemporalAnalysis, TemporalAssessment, TemporalDetection, TemporalInference,
    TemporalInferenceError, TemporalMatch,
};

pub fn detect(text: &str) -> TemporalDetection {
    reliquary_chronos::chronos::detect(text)
}

pub fn analyze(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalAnalysis {
    reliquary_chronos::chronos::analyze(text, reference_timestamp_ns)
}

pub fn assess(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalAssessment {
    reliquary_chronos::chronos::assess(text, reference_timestamp_ns)
}

pub fn analyze_with_inference(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    inference: &TemporalInference,
) -> Result<TemporalAnalysis, TemporalInferenceError> {
    reliquary_chronos::chronos::analyze_with_inference(text, reference_timestamp_ns, inference)
        .map_err(|error| match error {
            reliquary_chronos::TemporalInferenceError::InvalidOutput(message) => {
                TemporalInferenceError::InvalidOutput(message)
            }
        })
}

pub(crate) fn temporal_matches(
    source: &TemporalAnalysis,
    candidate: &TemporalAnalysis,
) -> (Vec<TemporalMatch>, f64) {
    reliquary_chronos::chronos::temporal_matches(source, candidate)
}
