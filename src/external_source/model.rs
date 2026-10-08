//! Host-owned canonical evidence. No adapter may use these IDs as Archive IDs.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    pub(crate) fn validate(&self, namespace: &str) -> Result<(), ExternalSourceError> {
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ExternalChangeKind {
    Upsert(ExternalMessageDescriptor),
    /// Rewind and compaction are host-authored visibility changes.
    Visibility {
        conversation_id: String,
        message_ids: Vec<String>,
        active: bool,
    },
    DeleteConversation {
        conversation_id: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalConversationChange {
    pub sequence: u64,
    pub kind: ExternalChangeKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalChangeBatch {
    /// Oldest retained event sequence; cursor+1 below this requires rebuild.
    pub feed_floor: u64,
    pub feed_watermark: u64,
    pub changes: Vec<ExternalConversationChange>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalSnapshotManifest {
    pub source_namespace: String,
    pub contract_version: u32,
    /// Exact host snapshot cut; subsequent changes start after this watermark.
    pub watermark: u64,
    pub messages: Vec<ExternalMessageDescriptor>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydratedExternalMessage {
    pub descriptor: ExternalMessageDescriptor,
    pub content: String,
}
impl HydratedExternalMessage {
    pub fn validate(
        &self,
        expected: &ExternalMessageDescriptor,
    ) -> Result<(), ExternalSourceError> {
        expected.validate(&expected.reference.source_namespace)?;
        if &self.descriptor != expected
            || !expected.active
            || self.content.len() > MAX_EXTERNAL_MESSAGE_BYTES
            || external_content_hash(&self.content) != expected.reference.content_hash
        {
            return Err(ExternalSourceError::StaleEvidence);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalSourceError {
    InvalidContract,
    Bounds,
    Unavailable,
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
            Self::Unauthorized => "external source access denied",
            Self::StaleEvidence => "external source evidence is stale",
            Self::RebuildRequired => "external source rebuild required",
            Self::Conflict => "external source cut changed; retry",
            Self::Storage => "external derived state unavailable",
        })
    }
}
impl std::error::Error for ExternalSourceError {}
pub fn external_content_hash(content: &str) -> [u8; 32] {
    Sha256::digest(content.as_bytes()).into()
}
pub(crate) fn identifier(value: &str) -> Result<(), ExternalSourceError> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(())
}

/// Implementations must authorize this verified principal for *every* feed,
/// manifest and hydration call. A REL grant never grants access to host text.
/// Calls run outside semantic locks and admission leases. Errors contain no bodies.
pub trait ExternalConversationSource: Send + Sync {
    fn source_namespace(&self) -> &str;
    fn authorize(&self, principal_id: &str) -> Result<(), ExternalSourceError>;
    fn contract_version(&self) -> u32 {
        EXTERNAL_SOURCE_CONTRACT
    }
    fn changes_after(
        &self,
        principal_id: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError>;
    fn snapshot_manifest(
        &self,
        principal_id: &str,
    ) -> Result<ExternalSnapshotManifest, ExternalSourceError>;
    fn hydrate(
        &self,
        principal_id: &str,
        messages: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalSearchHit {
    pub reference: ExternalChunkRef,
    pub score: f64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalIndexStatus {
    pub source_namespace: String,
    pub generation: u64,
    pub revision: u64,
    pub cursor: u64,
    pub messages: usize,
    pub chunks: usize,
    pub compatibility_profile: crate::CompatibilityProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HydratedExternalChunk {
    pub reference: ExternalChunkRef,
    pub content: String,
}
