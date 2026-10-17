use crate::{
    ArchiveError, ArchiveVectorError, CompatibilityProfileError, ContainerError,
    EmbeddingEndpointError, PackedVectorError, ScalarType,
};
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
