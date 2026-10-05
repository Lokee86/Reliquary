use crate::{
    DreamCandidateError, DreamClassificationError, DreamLifecycleError, DreamPublicationError,
    DreamVerificationError, MemoryError,
};
use std::fmt;

#[derive(Debug)]
pub enum DreamProcessError {
    Candidate(DreamCandidateError),
    Classification(DreamClassificationError),
    Verification(DreamVerificationError),
    Publication(DreamPublicationError),
    Lifecycle(DreamLifecycleError),
    Memory(MemoryError),
    Freshness(String),
}

impl fmt::Display for DreamProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Candidate(error) => write!(f, "{error}"),
            Self::Classification(error) => write!(f, "{error}"),
            Self::Verification(error) => write!(f, "{error}"),
            Self::Publication(error) => write!(f, "{error}"),
            Self::Lifecycle(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Freshness(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DreamProcessError {}

impl DreamProcessError {
    pub fn is_backpressure(&self) -> bool {
        match self {
            Self::Classification(error) => error.is_backpressure(),
            Self::Verification(error) => error.is_backpressure(),
            Self::Candidate(_)
            | Self::Publication(_)
            | Self::Lifecycle(_)
            | Self::Memory(_)
            | Self::Freshness(_) => false,
        }
    }
}

impl From<DreamCandidateError> for DreamProcessError {
    fn from(value: DreamCandidateError) -> Self {
        Self::Candidate(value)
    }
}
impl From<DreamClassificationError> for DreamProcessError {
    fn from(value: DreamClassificationError) -> Self {
        Self::Classification(value)
    }
}
impl From<DreamVerificationError> for DreamProcessError {
    fn from(value: DreamVerificationError) -> Self {
        Self::Verification(value)
    }
}
impl From<DreamPublicationError> for DreamProcessError {
    fn from(value: DreamPublicationError) -> Self {
        Self::Publication(value)
    }
}
impl From<DreamLifecycleError> for DreamProcessError {
    fn from(value: DreamLifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}
impl From<MemoryError> for DreamProcessError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}
