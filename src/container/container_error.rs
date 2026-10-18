use crate::ContainerError;
use std::fmt;

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "container I/O error: {error}"),
            Self::InvalidMagic => write!(f, "not a valid CVA container"),
            Self::TruncatedHeader => write!(f, "truncated CVA header"),
            Self::TruncatedChunk(offset) => write!(f, "truncated CVA chunk at {offset}"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported CVA format {}.{}", v.major, v.minor)
            }
            Self::InvalidHeaderLength(length) => write!(f, "invalid CVA header length {length}"),
            Self::InvalidChunkRef(_) => write!(f, "invalid CVA object reference"),
            Self::InvalidVersionRecord => write!(f, "invalid CVA version record"),
            Self::InvalidTransactionTime => write!(f, "invalid CVA transaction timestamp"),
            Self::VersionExhausted => write!(f, "CVA global version counter exhausted"),
            Self::ChunkTooLarge => write!(f, "CVA chunk is too large"),
            Self::InvalidIdentity => write!(f, "invalid container identity"),
            Self::WriteAdmissionClosed => write!(f, "container write admission is closed"),
        }
    }
}

impl std::error::Error for ContainerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
