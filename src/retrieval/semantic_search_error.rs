use crate::{
    ArchiveVectorError, CompatibilityProfileError, EmbeddingEndpointError, PackedVectorError,
    ScalarType,
};
use std::fmt;

#[derive(Debug)]
pub enum SemanticSearchError {
    Profile(CompatibilityProfileError),
    Endpoint(EmbeddingEndpointError),
    PackedVectors(PackedVectorError),
    ArchiveVectors(ArchiveVectorError),
    MissingProfile,
    MissingGeneration,
    MissingFragment,
    IncompatibleEndpoint,
    EmptyQuery,
    InvalidLimit,
    InvalidQueryVector,
    UnsupportedScalar(ScalarType),
    CorruptMatrix(&'static str),
}

impl fmt::Display for SemanticSearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "semantic-search error: {self:?}")
    }
}

impl std::error::Error for SemanticSearchError {}

impl From<CompatibilityProfileError> for SemanticSearchError {
    fn from(value: CompatibilityProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<EmbeddingEndpointError> for SemanticSearchError {
    fn from(value: EmbeddingEndpointError) -> Self {
        Self::Endpoint(value)
    }
}

impl From<PackedVectorError> for SemanticSearchError {
    fn from(value: PackedVectorError) -> Self {
        Self::PackedVectors(value)
    }
}

impl From<reliquary_vector_store::PackedVectorError> for SemanticSearchError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        Self::PackedVectors(value.into())
    }
}

impl From<ArchiveVectorError> for SemanticSearchError {
    fn from(value: ArchiveVectorError) -> Self {
        Self::ArchiveVectors(value)
    }
}

impl From<reliquary_vector_store::ArchiveVectorError> for SemanticSearchError {
    fn from(value: reliquary_vector_store::ArchiveVectorError) -> Self {
        Self::ArchiveVectors(value.into())
    }
}

impl From<reliquary_vectors::ProbeError> for SemanticSearchError {
    fn from(error: reliquary_vectors::ProbeError) -> Self {
        Self::Profile(error.into())
    }
}
