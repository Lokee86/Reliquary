use crate::ContainerError;
use std::fmt;

#[derive(Debug)]
pub enum EgoError {
    Container(ContainerError),
    Lifecycle(crate::owner_lifecycle::LifecycleError),
    InvalidRecord(String),
    InvalidOwnerRecord(&'static str),
    InvalidSourceMemoryVersion { source: u64, current: u64 },
    RevisionConflict { expected: u64, actual: u64 },
    NotFound(&'static str),
    ActiveIdentityDeletion,
}

impl fmt::Display for EgoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => write!(f, "Ego lifecycle error: {error:?}"),
            Self::Container(error) => write!(f, "{error}"),
            Self::InvalidRecord(message) => write!(f, "invalid Ego record: {message}"),
            Self::InvalidOwnerRecord(message) => write!(f, "invalid Ego owner record: {message}"),
            Self::InvalidSourceMemoryVersion { source, current } => write!(
                f,
                "invalid Ego source Memory version {source}: current owner Memory version is {current}"
            ),
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "Ego revision conflict: expected {expected}, actual {actual}"
            ),
            Self::NotFound(kind) => write!(f, "Ego {kind} was not found"),
            Self::ActiveIdentityDeletion => write!(
                f,
                "active Ego Identity cannot be deleted while another Identity exists"
            ),
        }
    }
}

impl std::error::Error for EgoError {}

impl From<ContainerError> for EgoError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

impl From<crate::owner_lifecycle::LifecycleError> for EgoError {
    fn from(value: crate::owner_lifecycle::LifecycleError) -> Self {
        Self::Lifecycle(value)
    }
}
