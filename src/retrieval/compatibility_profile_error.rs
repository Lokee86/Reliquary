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

impl From<reliquary_vector_store::ProfileStoreError> for CompatibilityProfileError {
    fn from(value: reliquary_vector_store::ProfileStoreError) -> Self {
        use reliquary_vector_store::ProfileStoreError as Store;
        match value {
            Store::Container(error) => Self::Container(error),
            Store::CorruptRecord(record) => Self::CorruptRecord(record),
            Store::MissingFormat => Self::MissingFormat,
            Store::ConflictingFormat => Self::ConflictingFormat,
            Store::MissingProfile => Self::MissingProfile,
            Store::InvalidProfile(reason) => Self::InvalidProfile(reason),
            Store::HashCollision => Self::HashCollision,
            Store::SizeOverflow => Self::SizeOverflow,
        }
    }
}
