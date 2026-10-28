use crate::{GraphError, MemoryEntityMentionKey, MemoryError};
use std::fmt;

#[derive(Debug)]
pub enum EntityCandidateError {
    InvalidConfig,
    MissingMention(MemoryEntityMentionKey),
    Memory(MemoryError),
    Graph(GraphError),
    Entity(crate::EntityError),
}

impl fmt::Display for EntityCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig => f.write_str("invalid Entity candidate retrieval config"),
            Self::MissingMention(key) => write!(
                f,
                "missing Entity mention for memory {:02x?} {:?} {}..{}",
                key.memory_id.0, key.field, key.start_byte, key.end_byte
            ),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::Entity(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EntityCandidateError {}

impl From<MemoryError> for EntityCandidateError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<GraphError> for EntityCandidateError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

impl From<crate::EntityError> for EntityCandidateError {
    fn from(value: crate::EntityError) -> Self {
        Self::Entity(value)
    }
}

impl From<reliquary_perception_store::EntityError> for EntityCandidateError {
    fn from(value: reliquary_perception_store::EntityError) -> Self {
        Self::Entity(value.into())
    }
}
