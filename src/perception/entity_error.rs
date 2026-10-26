//! Product-facing Entity error surface.
//!
//! Perception Store uses a local payload-availability fact so the store does
//! not depend on Owner Lifecycle. The root maps that fact back to the
//! established product error vocabulary here.

use reliquary_container::ContainerError;
use reliquary_perception_store::EntityPayloadUnavailable;
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
    PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable),
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

impl From<reliquary_perception_store::EntityError> for EntityError {
    fn from(value: reliquary_perception_store::EntityError) -> Self {
        use reliquary_perception_store::EntityError as StoreError;
        match value {
            StoreError::Container(error) => Self::Container(error),
            StoreError::CorruptRecord(value) => Self::CorruptRecord(value),
            StoreError::InvalidField(value) => Self::InvalidField(value),
            StoreError::InvalidUtf8 => Self::InvalidUtf8,
            StoreError::FieldTooLarge => Self::FieldTooLarge,
            StoreError::ConflictingFormat => Self::ConflictingFormat,
            StoreError::MissingEntity => Self::MissingEntity,
            StoreError::TerminalTombstone => Self::TerminalTombstone,
            StoreError::PayloadRequiredByLiveTarget => Self::PayloadRequiredByLiveTarget,
            StoreError::PayloadUnavailable(EntityPayloadUnavailable::Pruned) => {
                Self::PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable::Pruned)
            }
            StoreError::RevisionConflict => Self::RevisionConflict,
            StoreError::MutationConflict => Self::MutationConflict,
            StoreError::InvalidVersion => Self::InvalidVersion,
            StoreError::VersionExhausted => Self::VersionExhausted,
        }
    }
}
