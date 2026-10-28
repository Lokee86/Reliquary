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
