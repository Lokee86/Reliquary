use crate::PackedVectorError;
use reliquary_container::ContainerError;
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
    fn from(e: ContainerError) -> Self {
        Self::Container(e)
    }
}
impl From<PackedVectorError> for ArchiveVectorError {
    fn from(e: PackedVectorError) -> Self {
        Self::PackedVector(e)
    }
}
impl From<reliquary_vector_store::ArchiveVectorError> for ArchiveVectorError {
    fn from(value: reliquary_vector_store::ArchiveVectorError) -> Self {
        use reliquary_vector_store::ArchiveVectorError as E;
        match value {
            E::Container(e) => Self::Container(e),
            E::PackedVector(e) => Self::PackedVector(e.into()),
            E::CorruptRecord(m) => Self::CorruptRecord(m),
            E::MissingFormat => Self::MissingFormat,
            E::ConflictingFormat => Self::ConflictingFormat,
            E::MissingObject => Self::MissingObject,
            E::MissingPackedVector => Self::MissingPackedVector,
            E::RowCountMismatch => Self::RowCountMismatch,
            E::DuplicateFragment => Self::DuplicateFragment,
            E::HashCollision => Self::HashCollision,
            E::SizeOverflow => Self::SizeOverflow,
        }
    }
}
