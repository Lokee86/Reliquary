use reliquary_container::ContainerError;
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

impl From<reliquary_ego_store::EgoError> for EgoError {
    fn from(value: reliquary_ego_store::EgoError) -> Self {
        use reliquary_ego_store::EgoError as StoreError;

        match value {
            StoreError::Container(error) => Self::Container(error),
            StoreError::Lifecycle(error) => Self::Lifecycle(error.into()),
            StoreError::InvalidRecord(message) => Self::InvalidRecord(message),
            StoreError::InvalidOwnerRecord(message) => Self::InvalidOwnerRecord(message),
            StoreError::InvalidSourceMemoryVersion { source, current } => {
                Self::InvalidSourceMemoryVersion { source, current }
            }
            StoreError::RevisionConflict { expected, actual } => {
                Self::RevisionConflict { expected, actual }
            }
            StoreError::NotFound(kind) => Self::NotFound(kind),
            StoreError::ActiveIdentityDeletion => Self::ActiveIdentityDeletion,
        }
    }
}

impl From<reliquary_ego_store::owner_lifecycle::LifecycleError>
    for crate::owner_lifecycle::LifecycleError
{
    fn from(value: reliquary_ego_store::owner_lifecycle::LifecycleError) -> Self {
        use reliquary_ego_store::owner_lifecycle::{
            LifecycleError as StoreError, LifecycleTarget as StoreTarget,
        };

        match value {
            StoreError::TerminalTombstone(target) => Self::TerminalTombstone(match target {
                StoreTarget::EgoIdentity(id) => {
                    crate::owner_lifecycle::LifecycleTarget::EgoIdentity(id)
                }
                StoreTarget::EgoAnchor(id) => {
                    crate::owner_lifecycle::LifecycleTarget::EgoAnchor(id)
                }
            }),
        }
    }
}

impl From<reliquary_ego_store::owner_lifecycle::LifecycleState>
    for crate::owner_lifecycle::LifecycleState
{
    fn from(value: reliquary_ego_store::owner_lifecycle::LifecycleState) -> Self {
        use reliquary_ego_store::owner_lifecycle::LifecycleState as StoreState;

        match value {
            StoreState::Active => Self::Active,
            StoreState::Tombstoned(at) => {
                Self::Tombstoned(crate::owner_lifecycle::TombstoneCoordinates {
                    global_version: at.global_version,
                    domain_version: at.domain_version,
                })
            }
        }
    }
}
