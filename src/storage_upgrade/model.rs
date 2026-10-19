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
            Self::CopyMismatch => f.write_str("upgrade shell differs from frozen source"),
        }
    }
}
impl std::error::Error for StorageUpgradeError {}
