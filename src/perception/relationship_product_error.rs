use crate::{EntityId, MemoryId};
use reliquary_container::ContainerError;
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
    PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable),
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
impl From<reliquary_perception_store::RelationshipError> for RelationshipError {
    fn from(value: reliquary_perception_store::RelationshipError) -> Self {
        use reliquary_perception_store::RelationshipError as E;
        match value {
            E::Container(e) => Self::Container(e),
            E::CorruptRecord(v) => Self::CorruptRecord(v),
            E::InvalidField(v) => Self::InvalidField(v),
            E::InvalidUtf8 => Self::InvalidUtf8,
            E::FieldTooLarge => Self::FieldTooLarge,
            E::ConflictingFormat => Self::ConflictingFormat,
            E::MissingRelationship => Self::MissingRelationship,
            E::TerminalTombstone => Self::TerminalTombstone,
            E::PayloadRequiredByLiveTarget => Self::PayloadRequiredByLiveTarget,
            E::PayloadUnavailable(
                reliquary_perception_store::RelationshipPayloadUnavailable::Pruned,
            ) => Self::PayloadUnavailable(crate::owner_lifecycle::PayloadUnavailable::Pruned),
            E::MissingLocalEntity(id) => Self::MissingLocalEntity(id),
            E::MissingLocalMemory(id) => Self::MissingLocalMemory(MemoryId(id)),
            E::MissingOwnerIdentity => Self::MissingOwnerIdentity,
            E::RevisionConflict => Self::RevisionConflict,
            E::MutationConflict => Self::MutationConflict,
            E::InvalidVersion => Self::InvalidVersion,
            E::VersionExhausted => Self::VersionExhausted,
        }
    }
}
