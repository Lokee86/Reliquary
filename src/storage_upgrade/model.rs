use crate::{ContainerError, ContainerIdentity};
use std::{fmt, io, path::PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct StorageUpgradeRequest {
    pub source: PathBuf,
    pub destination: PathBuf,
}

/// Frozen source identity; callers cannot alter the accepted digest or paths.
#[derive(Clone, Debug)]
pub(crate) struct StorageUpgradePlan {
    pub(super) request: StorageUpgradeRequest,
    pub(super) source_digest: [u8; 32],
    pub(super) source_len: u64,
    pub(super) identity: ContainerIdentity,
    pub(super) owner_uuid: [u8; 16],
}

/// Mechanical shell evidence only; this is not semantic migration acceptance.
#[derive(Debug)]
pub(crate) struct StorageUpgradeResult {
    pub identity: ContainerIdentity,
    pub owner_uuid: [u8; 16],
    pub source_bytes: u64,
    pub global_head: u64,
}

#[derive(Debug)]
pub(crate) enum StorageUpgradeError {
    Io(io::Error),
    Container(ContainerError),
    OutputExists(PathBuf),
    SourceChanged,
    MissingOwnerIdentity,
    CopyMismatch,
    UnsupportedAuthority(&'static str),
    Memory(crate::MemoryError),
    Entity(crate::EntityError),
    Relationship(crate::RelationshipError),
    Import(crate::imports::ImportError),
    Migration(String),
    Owner(Box<crate::CvaError>),
}

impl From<crate::CvaError> for StorageUpgradeError {
    fn from(error: crate::CvaError) -> Self {
        Self::Owner(Box::new(error))
    }
}

impl From<crate::imports::ImportError> for StorageUpgradeError {
    fn from(error: crate::imports::ImportError) -> Self {
        Self::Import(error)
    }
}

impl From<crate::MemoryError> for StorageUpgradeError {
    fn from(error: crate::MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl From<crate::EntityError> for StorageUpgradeError {
    fn from(error: crate::EntityError) -> Self {
        Self::Entity(error)
    }
}

impl From<crate::RelationshipError> for StorageUpgradeError {
    fn from(error: crate::RelationshipError) -> Self {
        Self::Relationship(error)
    }
}

impl From<io::Error> for StorageUpgradeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<ContainerError> for StorageUpgradeError {
    fn from(error: ContainerError) -> Self {
        Self::Container(error)
    }
}
impl fmt::Display for StorageUpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "upgrade I/O: {error}"),
            Self::Container(error) => write!(f, "upgrade container: {error}"),
            Self::OutputExists(path) => {
                write!(f, "upgrade output already exists: {}", path.display())
            }
            Self::SourceChanged => f.write_str("upgrade source changed after planning"),
            Self::MissingOwnerIdentity => {
                f.write_str("upgrade requires retained owner identity and UUID")
            }
            Self::Owner(error) => write!(f, "upgrade owner: {error}"),
            Self::Import(error) => write!(f, "upgrade import proof: {error}"),
            Self::Memory(error) => write!(f, "upgrade Memory: {error}"),
            Self::Entity(error) => write!(f, "upgrade Entity: {error}"),
            Self::Relationship(error) => write!(f, "upgrade Relationship: {error}"),
            Self::Migration(error) => write!(f, "upgrade relocation: {error}"),
            Self::CopyMismatch => f.write_str("upgrade shell differs from frozen source"),
            Self::UnsupportedAuthority(family) => {
                write!(
                    f,
                    "upgrade has no canonical converter for authority {family}"
                )
            }
        }
    }
}
impl std::error::Error for StorageUpgradeError {}
