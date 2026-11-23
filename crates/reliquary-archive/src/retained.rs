//! Retained Archive facts carried into current owners without invented branch proof.
use crate::{ArchiveError, ContentId, Fragment, ProjectRevisionRef};

pub(crate) const RETAINED_MARKER: &[u8; 8] = b"RELART01";
pub const PROJECT_CORRELATION_MARKER: &[u8; 8] = b"RELPRC01";
pub const PROJECT_BACKING_MARKER: &[u8; 8] = b"RELPRB01";
pub(crate) const PROJECT_BACKING_PREFIX: &[u8] = b"RELPRB";
const PROJECT_BACKING_LENGTH: usize = 8 + 8 + 32 + 32;

/// The global frontier where an accepted project attachment gained backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectBackingBinding {
    pub global_version: u64,
    pub attachment_id: [u8; 32],
    pub content_id: ContentId,
}

pub(crate) fn decode_project_backing(bytes: &[u8]) -> Result<ProjectBackingBinding, ArchiveError> {
    if bytes.len() != PROJECT_BACKING_LENGTH || !bytes.starts_with(PROJECT_BACKING_MARKER) {
        return Err(ArchiveError::CorruptRecord(
            "invalid project backing record",
        ));
    }
    Ok(ProjectBackingBinding {
        global_version: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        attachment_id: bytes[16..48].try_into().unwrap(),
        content_id: ContentId(bytes[48..80].try_into().unwrap()),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationPolicy {
    SequentialChronological,
    VisibleAutoBranch,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RetainedArchiveValue {
    ConversationMetadata {
        conversation_id: String,
        title: Option<String>,
        active: bool,
        policy: ConversationPolicy,
        policy_revision: u64,
    },
    Fragment {
        fragment: Fragment,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetainedArchivePublication {
    pub archive_version: u64,
    pub value: RetainedArchiveValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryManagement {
    WarlockManaged,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCorrelation {
    pub sequence: u64,
    pub global_version: u64,
    pub archive_version: u64,
    pub memory_version: u64,
    pub repository_management: RepositoryManagement,
    pub project_revision: ProjectRevisionRef,
}

pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ArchiveError> {
    serde_json::from_slice(&bytes[8..])
        .map_err(|_| ArchiveError::CorruptRecord("invalid retained Archive value"))
}
