use crate::{
    ArchiveError, InsomniaError, InsomniaExtractionError, InsomniaRejection, Memory, MemoryError,
};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaProcessResult {
    pub created: Vec<Memory>,
    pub existing: Vec<Memory>,
    pub user_created: Vec<Memory>,
    pub user_existing: Vec<Memory>,
    pub rejected: Vec<InsomniaRejection>,
}

#[derive(Debug)]
pub enum InsomniaProcessError {
    MissingEpisode,
    InvalidClaim,
    InvalidCandidate(String),
    UserRoutingRequired,
    Phylactery(String),
    Extraction(InsomniaExtractionError),
    Memory(MemoryError),
    Queue(InsomniaError),
    Archive(ArchiveError),
}

impl fmt::Display for InsomniaProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEpisode => write!(f, "Insomnia episode is missing"),
            Self::InvalidClaim => write!(f, "Insomnia work is not an active claim"),
            Self::InvalidCandidate(message) => write!(f, "invalid Insomnia candidate: {message}"),
            Self::UserRoutingRequired => write!(
                f,
                "Insomnia produced user-owned Memory but no Phylactery routing target was supplied"
            ),
            Self::Phylactery(message) => write!(f, "Phylactery routing failed: {message}"),
            Self::Extraction(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Queue(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for InsomniaProcessError {}

impl From<InsomniaExtractionError> for InsomniaProcessError {
    fn from(value: InsomniaExtractionError) -> Self {
        Self::Extraction(value)
    }
}

impl From<MemoryError> for InsomniaProcessError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<InsomniaError> for InsomniaProcessError {
    fn from(value: InsomniaError) -> Self {
        Self::Queue(value)
    }
}

impl From<ArchiveError> for InsomniaProcessError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}
