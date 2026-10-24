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

impl From<crate::memory_vector_reader::MemoryVectorReadError> for DreamCandidateError {
    fn from(value: crate::memory_vector_reader::MemoryVectorReadError) -> Self {
        use crate::memory_vector_reader::MemoryVectorReadError;
        match value {
            MemoryVectorReadError::MemoryVectors(error) => Self::MemoryVectors(error),
            MemoryVectorReadError::PackedVectors(error) => Self::PackedVectors(error),
            MemoryVectorReadError::UnsupportedScalar(scalar) => Self::UnsupportedScalar(scalar),
            MemoryVectorReadError::CorruptVector(reason) => Self::CorruptVector(reason),
        }
    }
}

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
