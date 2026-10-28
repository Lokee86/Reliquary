use serde::{Deserialize, Serialize};
use std::fmt;
pub const EXTERNAL_SOURCE_CONTRACT: u32 = 1;
pub const MAX_EXTERNAL_BATCH: usize = 64;
pub const MAX_EXTERNAL_MANIFEST: usize = 8192;
pub const MAX_EXTERNAL_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_EXTERNAL_INDEX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_EXTERNAL_RESULTS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalMessageRef {
    pub source_namespace: String,
    pub conversation_id: String,
    pub message_id: String,
    pub content_hash: [u8; 32],
}
impl ExternalMessageRef {
    pub fn validate(&self) -> Result<(), ExternalSourceError> {
        for id in [
            &self.source_namespace,
            &self.conversation_id,
            &self.message_id,
        ] {
            identifier(id)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalChunkRef {
    pub message: ExternalMessageRef,
    /// UTF-8 byte offsets, validated against freshly hydrated canonical content.
    pub start: u64,
    pub end: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalMessageDescriptor {
    pub reference: ExternalMessageRef,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub active: bool,
}
impl ExternalMessageDescriptor {
    pub fn validate(&self, namespace: &str) -> Result<(), ExternalSourceError> {
        self.reference.validate()?;
        if let Some(principal) = &self.principal_id {
            identifier(principal)?;
        }
        if self.reference.source_namespace != namespace
            || !matches!(self.role.as_str(), "user" | "assistant" | "system" | "tool")
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalSourceError {
    InvalidContract,
    Bounds,
    Unavailable,
    TemporalDecisionUnavailable,
    TemporalDecisionInvalidOutput,
    TemporalDecisionBackpressure { retry_after_ns: Option<u64> },
    Unauthorized,
    StaleEvidence,
    RebuildRequired,
    Conflict,
    Storage,
}
impl fmt::Display for ExternalSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidContract => "invalid external source contract",
            Self::Bounds => "external source bound exceeded",
            Self::Unavailable => "external source unavailable",
            Self::TemporalDecisionUnavailable => "temporal decision unavailable",
            Self::TemporalDecisionInvalidOutput => "invalid temporal decision output",
            Self::TemporalDecisionBackpressure { .. } => "temporal decision backpressure",
            Self::Unauthorized => "external source access denied",
            Self::StaleEvidence => "external source evidence is stale",
            Self::RebuildRequired => "external source rebuild required",
            Self::Conflict => "external source cut changed; retry",
            Self::Storage => "external derived state unavailable",
        })
    }
}
impl std::error::Error for ExternalSourceError {}
pub fn identifier(value: &str) -> Result<(), ExternalSourceError> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(())
}
