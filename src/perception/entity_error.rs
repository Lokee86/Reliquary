use crate::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum EntityError {
    Container(ContainerError),
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    InvalidUtf8,
    FieldTooLarge,
    ConflictingFormat,
    MissingEntity,
    TerminalTombstone,
    PayloadRequiredByLiveTarget,
    RevisionConflict,
    MutationConflict,
    InvalidVersion,
    VersionExhausted,
}

impl fmt::Display for EntityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity error: {self:?}")
    }
}

impl std::error::Error for EntityError {}

impl From<ContainerError> for EntityError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
