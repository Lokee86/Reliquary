use crate::{
    CompatibilityProfileError, EmbeddingEndpointError, MemoryError, PackedVectorError, ScalarType,
};
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum MemoryVectorError {
    Container(ContainerError),
    PackedVector(PackedVectorError),
    Profile(CompatibilityProfileError),
    Memory(MemoryError),
    Endpoint(EmbeddingEndpointError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    MissingProfile,
    MissingMemoryBody,
    DuplicateMemoryBody,
    DuplicateBinding,
    RowCountMismatch,
    DimensionMismatch,
    UnsupportedScalar(ScalarType),
    IncompatibleEndpoint,
    InvalidEmbeddingMatrix,
    EmptyPopulation,
    HashCollision,
    SizeOverflow,
}

impl fmt::Display for MemoryVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "memory-vector error: {self:?}")
    }
}

impl std::error::Error for MemoryVectorError {}

impl From<ContainerError> for MemoryVectorError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<PackedVectorError> for MemoryVectorError {
    fn from(value: PackedVectorError) -> Self {
        Self::PackedVector(value)
    }
}

impl From<reliquary_vector_store::PackedVectorError> for MemoryVectorError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        Self::PackedVector(value.into())
    }
}

impl From<CompatibilityProfileError> for MemoryVectorError {
    fn from(value: CompatibilityProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<MemoryError> for MemoryVectorError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<EmbeddingEndpointError> for MemoryVectorError {
    fn from(value: EmbeddingEndpointError) -> Self {
        Self::Endpoint(value)
    }
}
