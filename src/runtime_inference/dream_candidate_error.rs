use crate::{MemoryError, MemoryVectorError, PackedVectorError, ScalarType};
use std::fmt;

#[derive(Debug)]
pub enum DreamCandidateError {
    Memory(MemoryError),
    MemoryVectors(MemoryVectorError),
    PackedVectors(PackedVectorError),
    InvalidConfig,
    MissingSourceVector,
    UnsupportedScalar(ScalarType),
    CorruptVector(&'static str),
}

impl fmt::Display for DreamCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dream-candidate error: {self:?}")
    }
}

impl std::error::Error for DreamCandidateError {}

impl From<MemoryError> for DreamCandidateError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<MemoryVectorError> for DreamCandidateError {
    fn from(value: MemoryVectorError) -> Self {
        Self::MemoryVectors(value)
    }
}

impl From<PackedVectorError> for DreamCandidateError {
    fn from(value: PackedVectorError) -> Self {
        Self::PackedVectors(value)
    }
}

impl From<reliquary_vector_store::PackedVectorError> for DreamCandidateError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        Self::PackedVectors(value.into())
    }
}
