use crate::{CredentialError, MasterKeyError};
use std::{fmt, io};

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    InvalidMagic,
    UnsupportedVersion { major: u16, minor: u16 },
    Truncated,
    InvalidObject,
    DuplicateObject(String),
    InvalidFragmentConfig,
    InvalidRetrievalConfig,
    InvalidRuntimeConfig,
    InvalidModelSwitchboard,
    Credential(CredentialError),
    MasterKey(MasterKeyError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "configuration I/O error: {error}"),
            Self::InvalidMagic => write!(f, "not a Reliquary configuration file"),
            Self::UnsupportedVersion { major, minor } => {
                write!(
                    f,
                    "unsupported Reliquary configuration format {major}.{minor}"
                )
            }
            Self::Truncated => write!(f, "truncated Reliquary configuration file"),
            Self::InvalidObject => write!(f, "invalid Reliquary configuration object"),
            Self::DuplicateObject(key) => write!(f, "duplicate configuration object: {key}"),
            Self::InvalidFragmentConfig => write!(f, "invalid fragment configuration"),
            Self::InvalidRetrievalConfig => write!(f, "invalid retrieval configuration"),
            Self::InvalidRuntimeConfig => write!(f, "invalid runtime configuration"),
            Self::InvalidModelSwitchboard => write!(f, "invalid model switchboard configuration"),
            Self::Credential(error) => error.fmt(f),
            Self::MasterKey(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<CredentialError> for ConfigError {
    fn from(value: CredentialError) -> Self {
        Self::Credential(value)
    }
}

impl From<MasterKeyError> for ConfigError {
    fn from(value: MasterKeyError) -> Self {
        Self::MasterKey(value)
    }
}

impl From<reliquary_switchboard::ConfigDocumentError> for ConfigError {
    fn from(value: reliquary_switchboard::ConfigDocumentError) -> Self {
        use reliquary_switchboard::ConfigDocumentError as Raw;
        match value {
            Raw::Io(error) => Self::Io(error),
            Raw::InvalidMagic => Self::InvalidMagic,
            Raw::UnsupportedVersion { major, minor } => Self::UnsupportedVersion { major, minor },
            Raw::Truncated => Self::Truncated,
            Raw::InvalidObject => Self::InvalidObject,
            Raw::DuplicateObject(key) => Self::DuplicateObject(key),
        }
    }
}

impl From<reliquary_switchboard::CredentialDocumentError> for ConfigError {
    fn from(value: reliquary_switchboard::CredentialDocumentError) -> Self {
        match value {
            reliquary_switchboard::CredentialDocumentError::Credential(error) => {
                Self::Credential(error)
            }
            reliquary_switchboard::CredentialDocumentError::MasterKey(error) => {
                Self::MasterKey(error)
            }
        }
    }
}

impl From<reliquary_switchboard::ModelConfigError> for ConfigError {
    fn from(value: reliquary_switchboard::ModelConfigError) -> Self {
        match value {
            reliquary_switchboard::ModelConfigError::InvalidModelSwitchboard => {
                Self::InvalidModelSwitchboard
            }
            reliquary_switchboard::ModelConfigError::Credential(error) => Self::Credential(error),
        }
    }
}
