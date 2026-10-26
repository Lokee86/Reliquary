use crate::EmbeddingEndpointError;
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum CompatibilityProfileError {
    Container(ContainerError),
    Endpoint(EmbeddingEndpointError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingProfile,
    InvalidProfile(&'static str),
    HashCollision,
    SizeOverflow,
}

impl fmt::Display for CompatibilityProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compatibility-profile error: {self:?}")
    }
}

impl std::error::Error for CompatibilityProfileError {}

impl From<ContainerError> for CompatibilityProfileError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<EmbeddingEndpointError> for CompatibilityProfileError {
    fn from(value: EmbeddingEndpointError) -> Self {
        Self::Endpoint(value)
    }
}
