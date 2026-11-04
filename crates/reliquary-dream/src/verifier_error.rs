use reliquary_switchboard::GeneralEndpointError;
use std::fmt;

#[derive(Debug)]
pub enum DreamVerificationError {
    Endpoint(GeneralEndpointError),
    InvalidPair,
    NoRelation,
    InvalidOutput(String),
}

impl fmt::Display for DreamVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::InvalidPair => write!(f, "Dream verification pair does not match classification"),
            Self::NoRelation => write!(f, "Dream verification requires a non-none relation"),
            Self::InvalidOutput(message) => write!(f, "invalid Dream verification: {message}"),
        }
    }
}

impl std::error::Error for DreamVerificationError {}

impl DreamVerificationError {
    pub fn is_backpressure(&self) -> bool {
        matches!(self, Self::Endpoint(error) if error.is_backpressure())
    }
}

impl From<GeneralEndpointError> for DreamVerificationError {
    fn from(value: GeneralEndpointError) -> Self {
        Self::Endpoint(value)
    }
}
