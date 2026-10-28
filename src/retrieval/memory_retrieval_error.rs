use crate::{MemoryError, MemoryVectorError, PackedVectorError, ScalarType};
use std::fmt;

#[derive(Debug)]
pub enum MemoryRetrievalError {
    Memory(MemoryError),
    MemoryVectors(MemoryVectorError),
    PackedVectors(PackedVectorError),
    InvalidConfig,
    InvalidQueryVector,
    EmptyPopulation,
    StaleIndex,
    UnsupportedScalar(ScalarType),
    CorruptVector(&'static str),
}

impl fmt::Display for MemoryRetrievalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "memory-retrieval error: {self:?}")
    }
}

impl std::error::Error for MemoryRetrievalError {}

impl From<MemoryError> for MemoryRetrievalError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<MemoryVectorError> for MemoryRetrievalError {
    fn from(value: MemoryVectorError) -> Self {
        Self::MemoryVectors(value)
    }
}

impl From<PackedVectorError> for MemoryRetrievalError {
    fn from(value: PackedVectorError) -> Self {
        Self::PackedVectors(value)
    }
}

impl From<reliquary_vector_store::PackedVectorError> for MemoryRetrievalError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        Self::PackedVectors(value.into())
    }
}

impl From<reliquary_retrieval::memory_retrieval::RetrievalPolicyError> for MemoryRetrievalError {
    fn from(value: reliquary_retrieval::memory_retrieval::RetrievalPolicyError) -> Self {
        use reliquary_retrieval::memory_retrieval::RetrievalPolicyError as Policy;
        match value {
            Policy::InvalidConfig => Self::InvalidConfig,
            Policy::InvalidQueryVector => Self::InvalidQueryVector,
            Policy::EmptyPopulation => Self::EmptyPopulation,
            Policy::StaleIndex => Self::StaleIndex,
            Policy::CorruptVector(reason) => Self::CorruptVector(reason),
        }
    }
}
