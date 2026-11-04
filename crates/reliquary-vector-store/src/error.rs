use lodestone_packed::ScalarType;
use reliquary_container::ContainerError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadUnavailable {
    Pruned,
}

#[derive(Debug)]
pub enum PackedVectorError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    PayloadUnavailable(PayloadUnavailable),
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

#[derive(Debug)]
pub enum ArchiveVectorError {
    Container(ContainerError),
    PackedVector(PackedVectorError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    RowCountMismatch,
    DuplicateFragment,
    HashCollision,
    SizeOverflow,
}
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
impl fmt::Display for ArchiveVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "archive vector store error: {self:?}")
    }
}
impl std::error::Error for ArchiveVectorError {}

#[derive(Debug)]
pub enum MemoryVectorError {
    Container(ContainerError),
    PackedVector(PackedVectorError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    MissingProfile,
    RowCountMismatch,
    DimensionMismatch,
    UnsupportedScalar(ScalarType),
    DuplicateMemoryBody,
    DuplicateBinding,
    HashCollision,
    SizeOverflow,
}
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
impl fmt::Display for MemoryVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "memory vector store error: {self:?}")
    }
}
impl std::error::Error for MemoryVectorError {}

#[derive(Debug)]
pub enum GenerationStoreError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingProfile,
    MissingArchiveVector,
    InvalidGenerationVersion,
    SourceVersionRegression,
    VectorVersionExhausted,
    HashCollision,
    SizeOverflow,
}
impl From<ContainerError> for GenerationStoreError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
impl fmt::Display for GenerationStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vector generation store error: {self:?}")
    }
}
impl std::error::Error for GenerationStoreError {}
