use crate::{ContainerError, EntityId, MemoryId};
use std::fmt;

#[derive(Debug)]
pub enum RelationshipError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    InvalidUtf8,
    FieldTooLarge,
    ConflictingFormat,
    MissingRelationship,
    TerminalTombstone,
    PayloadRequiredByLiveTarget,
    MissingLocalEntity(EntityId),
    MissingLocalMemory(MemoryId),
    MissingOwnerIdentity,
    RevisionConflict,
    MutationConflict,
    InvalidVersion,
    VersionExhausted,
}

impl fmt::Display for RelationshipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "relationship error: {self:?}")
    }
}

impl std::error::Error for RelationshipError {}

impl From<ContainerError> for RelationshipError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
