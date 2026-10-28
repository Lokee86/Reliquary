use crate::{EntityResolverError, GeneralEndpointError};
use std::fmt;

#[derive(Debug)]
pub enum EntityReconciliationError {
    Endpoint(GeneralEndpointError),
    Resolver(EntityResolverError),
    Operation(String),
    InvalidOutput(String),
}

impl fmt::Display for EntityReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Endpoint(error) => write!(f, "{error}"),
            Self::Resolver(error) => write!(f, "{error}"),
            Self::Operation(error) => write!(f, "Entity reconciliation operation: {error}"),
            Self::InvalidOutput(error) => write!(f, "Entity reconciliation output: {error}"),
        }
    }
}

impl std::error::Error for EntityReconciliationError {}

impl From<GeneralEndpointError> for EntityReconciliationError {
    fn from(value: GeneralEndpointError) -> Self {
        Self::Endpoint(value)
    }
}

impl From<EntityResolverError> for EntityReconciliationError {
    fn from(value: EntityResolverError) -> Self {
        Self::Resolver(value)
    }
}

impl From<reliquary_entity_resolution::ReconciliationError> for EntityReconciliationError {
    fn from(value: reliquary_entity_resolution::ReconciliationError) -> Self {
        match value {
            reliquary_entity_resolution::ReconciliationError::Endpoint(e) => Self::Endpoint(e),
            reliquary_entity_resolution::ReconciliationError::InvalidOutput(s) => {
                Self::InvalidOutput(s)
            }
        }
    }
}
