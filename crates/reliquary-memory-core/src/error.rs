use reliquary_container::ContainerError;
use std::{fmt, io};

#[derive(Debug)]
pub enum MemoryError {
    Container(ContainerError),
    Io(io::Error),
    Corrupt(&'static str),
    MissingMemory,
    WrongOwner,
    RevisionConflict,
    MutationConflict,
    ImmutableBodyChanged,
    TerminalTombstone,
    FieldTooLarge,
    HashCollision,
    VersionExhausted,
    InvalidField(&'static str),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(e) => write!(f, "container: {e}"),
            Self::Io(e) => write!(f, "Memory I/O: {e}"),
            Self::Corrupt(reason) => write!(f, "corrupt Memory record: {reason}"),
            Self::WrongOwner => f.write_str("Memory store belongs to a different owner"),
            Self::MissingMemory => f.write_str("Memory identity is absent"),
            Self::RevisionConflict => {
                f.write_str("Memory revision does not match the expected predecessor")
            }
            Self::MutationConflict => {
                f.write_str("mutation identity was reused with different content")
            }
            Self::ImmutableBodyChanged => {
                f.write_str("Memory body identity cannot change across revisions")
            }
            Self::TerminalTombstone => f.write_str("Memory is terminally tombstoned"),
            Self::FieldTooLarge => f.write_str("Memory field exceeds the supported size"),
            Self::HashCollision => f.write_str("body identity resolves to different body bytes"),
            Self::VersionExhausted => f.write_str("Memory owner version exhausted"),
            Self::InvalidField(field) => write!(f, "invalid Memory field: {field}"),
        }
    }
}

impl std::error::Error for MemoryError {}
impl From<ContainerError> for MemoryError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}
impl From<io::Error> for MemoryError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
