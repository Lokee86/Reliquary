use crate::{
    ArchiveError, ArchiveVectorError, CompatibilityProfileError, EmbeddingEndpointError,
    PackedVectorError, ScalarType,
};
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum VectorGenerationError {
    Container(ContainerError),
    Freshness(String),
    Archive(ArchiveError),
    PackedVectors(PackedVectorError),
    ArchiveVectors(ArchiveVectorError),
    Profile(CompatibilityProfileError),
    Endpoint(EmbeddingEndpointError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingGeneration,
    MissingProfile,
    MissingArchiveVectors,
    IncompatibleEndpoint,
    DimensionMismatch,
    UnsupportedScalar(ScalarType),
    SourceArchiveVersion,
    SourceVersionRegression,
    EmptyPopulation,
    InvalidEmbeddingMatrix,
    IncompleteCoverage,
    InvalidGenerationVersion,
    VectorVersionExhausted,
    HashCollision,
}

impl fmt::Display for VectorGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vector-generation error: {self:?}")
    }
}

impl std::error::Error for VectorGenerationError {}

impl From<reliquary_vector_store::generation::GenerationStoreError> for VectorGenerationError {
    fn from(value: reliquary_vector_store::generation::GenerationStoreError) -> Self {
        use reliquary_vector_store::generation::GenerationStoreError as StoreError;
        match value {
            StoreError::Container(error) => Self::Container(error),
            StoreError::CorruptRecord(message) => Self::CorruptRecord(message),
            StoreError::MissingFormat => Self::MissingFormat,
            StoreError::ConflictingFormat => Self::ConflictingFormat,
            StoreError::InvalidGenerationVersion => Self::InvalidGenerationVersion,
            StoreError::SourceVersionRegression => Self::SourceVersionRegression,
            StoreError::VectorVersionExhausted => Self::VectorVersionExhausted,
            StoreError::HashCollision => Self::HashCollision,
        }
    }
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for VectorGenerationError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}

from_error!(ContainerError, Container);
from_error!(ArchiveError, Archive);
from_error!(PackedVectorError, PackedVectors);
from_error!(ArchiveVectorError, ArchiveVectors);
from_error!(CompatibilityProfileError, Profile);
from_error!(EmbeddingEndpointError, Endpoint);
