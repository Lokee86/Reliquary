//! Import epoch owner records and public error values; no service policy.
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportWatermarks {
    pub global_version: u64,
    pub archive_version: u64,
    pub memory_version: u64,
    pub entity_version: u64,
    pub relationship_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    pub vector_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportStart {
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub import_key: String,
    pub source_namespace: String,
    pub source_format: String,
    pub started_at_ns: i64,
    pub pre: ImportWatermarks,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportComplete {
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub start_digest: String,
    pub receipt_digest: String,
    pub completed_at_ns: i64,
    pub post: ImportWatermarks,
    pub historical_freshness_settlement_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportEpochStatus {
    Open(ImportStart),
    Completed {
        start: ImportStart,
        complete: ImportComplete,
    },
}

#[derive(Debug)]
pub enum ImportError {
    Invalid(&'static str),
    Unsupported(&'static str),
    Json(serde_json::Error),
    Archive(crate::ArchiveError),
    Insomnia(crate::InsomniaError),
    Container(crate::ContainerError),
    Epoch(&'static str),
}
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native import error: {self:?}")
    }
}
impl std::error::Error for ImportError {}
impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<crate::ArchiveError> for ImportError {
    fn from(e: crate::ArchiveError) -> Self {
        Self::Archive(e)
    }
}
impl From<crate::InsomniaError> for ImportError {
    fn from(e: crate::InsomniaError) -> Self {
        Self::Insomnia(e)
    }
}
impl From<crate::ContainerError> for ImportError {
    fn from(e: crate::ContainerError) -> Self {
        Self::Container(e)
    }
}
