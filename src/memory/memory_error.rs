use crate::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum MemoryError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    InvalidUtf8,
    FieldTooLarge,
    MissingFormat,
    ConflictingFormat,
    MissingBody,
    CorruptBody,
    HashCollision,
    RevisionConflict,
    SemanticMutation,
    MissingMemory,
    MutationConflict,
    RoutingMetadataConflict,
    InvalidProvenance,
    InvalidVersion,
    Freshness(String),
    VersionExhausted,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Freshness(error) => write!(f, "memory Freshness initialization failed: {error}"),
            _ => write!(f, "memory error: {self:?}"),
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<ContainerError> for MemoryError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
