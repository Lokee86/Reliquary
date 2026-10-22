//! Archive-owned retained evidence for imported source history.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) fn receipt_digest(receipt: &ImportReceipt) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(receipt)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReceipt {
    pub schema_version: u32,
    pub import_key: String,
    pub source_namespace: String,
    pub source_format: String,
    pub conversations: Vec<ImportedConversation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedConversation {
    pub source_id: String,
    pub conversation_id: String,
    pub title: Option<String>,
    pub messages: Vec<ImportedMessage>,
    pub branches: Vec<ImportedBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedMessage {
    pub source_id: String,
    pub revision: String,
    pub source_parent_id: Option<String>,
    pub replaces: Option<String>,
    pub deleted: bool,
    pub node_id: Option<String>,
    pub fingerprint: String,
    pub artifacts: Vec<ImportedArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedArtifact {
    pub source_id: String,
    pub source_ref: String,
    pub sha256: String,
    pub file_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedBranch {
    pub source_id: String,
    pub branch_id: String,
    /// Empty/deleted branch has no native head; its source evidence is still retained.
    pub leaf_node_id: Option<String>,
    pub canonical: bool,
}
