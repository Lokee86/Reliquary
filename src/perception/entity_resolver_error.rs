use crate::{EntityCandidateError, EntityError, GeneralEndpointError, GraphError, MemoryError};
use std::fmt;

#[derive(Debug)]
pub enum EntityResolverError {
    Candidate(EntityCandidateError),
    Endpoint(GeneralEndpointError),
    Memory(MemoryError),
    Entity(EntityError),
    Graph(GraphError),
    InvalidOutput(String),
}

impl fmt::Display for EntityResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Candidate(error) => write!(f, "{error}"),
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Entity(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "{error}"),
            Self::InvalidOutput(message) => write!(f, "Entity resolver output: {message}"),
        }
    }
}

impl std::error::Error for EntityResolverError {}

impl From<EntityCandidateError> for EntityResolverError {
    fn from(value: EntityCandidateError) -> Self {
        Self::Candidate(value)
    }
}

impl From<GeneralEndpointError> for EntityResolverError {
    fn from(value: GeneralEndpointError) -> Self {
        Self::Endpoint(value)
    }
}

impl From<MemoryError> for EntityResolverError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl From<EntityError> for EntityResolverError {
    fn from(value: EntityError) -> Self {
        Self::Entity(value)
    }
}

impl From<reliquary_perception_store::EntityError> for EntityResolverError {
    fn from(value: reliquary_perception_store::EntityError) -> Self {
        Self::Entity(value.into())
    }
}

impl From<GraphError> for EntityResolverError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

impl From<reliquary_entity_resolution::EntityResolverError> for EntityResolverError {
    fn from(value: reliquary_entity_resolution::EntityResolverError) -> Self {
        match value {
            reliquary_entity_resolution::EntityResolverError::Endpoint(e) => Self::Endpoint(e),
            reliquary_entity_resolution::EntityResolverError::InvalidOutput(s) => {
                Self::InvalidOutput(s)
            }
        }
    }
}
