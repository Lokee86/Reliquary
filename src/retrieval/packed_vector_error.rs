use reliquary_container::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum PackedVectorError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable),
    HashCollision,
    SizeOverflow,
}

impl fmt::Display for PackedVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "packed-vector error: {self:?}")
    }
}

impl std::error::Error for PackedVectorError {}

impl From<ContainerError> for PackedVectorError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<reliquary_vector_store::PackedVectorError> for PackedVectorError {
    fn from(value: reliquary_vector_store::PackedVectorError) -> Self {
        use reliquary_vector_store::PackedVectorError as StoreError;
        match value {
            StoreError::Container(error) => Self::Container(error),
            StoreError::CorruptRecord(message) => Self::CorruptRecord(message),
            StoreError::MissingFormat => Self::MissingFormat,
            StoreError::ConflictingFormat => Self::ConflictingFormat,
            StoreError::MissingObject => Self::MissingObject,
            StoreError::PayloadUnavailable(_) => {
                Self::PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable::Pruned)
            }
            StoreError::HashCollision => Self::HashCollision,
            StoreError::SizeOverflow => Self::SizeOverflow,
        }
    }
}
