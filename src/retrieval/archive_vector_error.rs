use crate::{ContainerError, PackedVectorError};
use std::fmt;

#[derive(Debug)]
pub enum ArchiveVectorError {
    Container(ContainerError),
    PackedVector(PackedVectorError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    MissingFragment,
    DuplicateFragment,
    RowCountMismatch,
    HashCollision,
    SizeOverflow,
}

impl fmt::Display for ArchiveVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "archive-vector error: {self:?}")
    }
}

impl std::error::Error for ArchiveVectorError {}

impl From<ContainerError> for ArchiveVectorError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<PackedVectorError> for ArchiveVectorError {
    fn from(value: PackedVectorError) -> Self {
        Self::PackedVector(value)
    }
}
