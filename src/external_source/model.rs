//! Host-owned canonical evidence. No adapter may use these IDs as Archive IDs.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use reliquary_external_index_store::model::*;
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
pub fn external_content_hash(content: &str) -> [u8; 32] {
    Sha256::digest(content.as_bytes()).into()
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
