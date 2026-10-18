use crate::ContainerError;
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
