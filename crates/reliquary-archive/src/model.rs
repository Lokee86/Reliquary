use reliquary_container::ContainerError;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EpisodeId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub id: String,
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content_id: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTombstone {
    pub conversation_id: String,
    pub archive_version: u64,
}

/// Current owner-local display title for one conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTitle {
    pub conversation_id: String,
    pub title: String,
    pub archive_version: u64,
}

pub const HERMES_COPIED_SOURCE_FORMAT: &str = "hermes-source-copied-snapshot-v1";

/// Source-to-native mapping receipt for one complete normalized import.
/// Transcript and artifact bodies remain in their ordinary Archive records.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReceipt {
    pub schema_version: u32,
    pub owner_uuid: [u8; 16],
    pub import_key: [u8; 32],
    pub source_namespace: String,
    pub source_format: String,
    pub source_digest: [u8; 32],
    pub conversations: Vec<ImportedConversation>,
    pub receipt_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedConversation {
    pub source_id: String,
    pub conversation_id: String,
    pub title: Option<String>,
    pub messages: Vec<ImportedMessage>,
    pub branches: Vec<ImportedBranch>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedMessage {
    pub source_id: String,
    pub revision: String,
    pub source_parent_id: Option<String>,
    pub replaces: Option<String>,
    pub deleted: bool,
    pub node_id: Option<String>,
    pub fingerprint: [u8; 32],
    pub artifacts: Vec<ImportedArtifact>,
    /// Source-provided identity. The trusted host authorizes routing separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_author: Option<ImportedSourceAuthor>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedSourceAuthor {
    pub id: Option<String>,
    pub is_bot: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedBranch {
    pub source_id: String,
    pub branch_id: String,
    pub leaf_node_id: Option<String>,
    pub canonical: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedArtifact {
    pub source_id: String,
    pub source_ref: String,
    pub sha256: [u8; 32],
    pub file_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnInput {
    pub id: String,
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
    pub attachments: Vec<AttachmentInput>,
    pub project_attachments: Vec<ProjectAttachmentInput>,
}

/// Attachment bytes supplied with a source turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentInput {
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectRepositoryKind {
    Lore,
    Git,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectRepositoryRef {
    pub kind: ProjectRepositoryKind,
    pub repository_id: String,
    pub project_path: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectRevisionRef {
    pub repository: ProjectRepositoryRef,
    pub revision: String,
}

/// Immutable provenance metadata; the repository remains authoritative for
/// reading the referenced contents at this revision.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectFileRef {
    pub revision: ProjectRevisionRef,
    pub path: String,
    pub content_hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAttachmentInput {
    pub filename: String,
    pub mime_type: Option<String>,
    pub content_id: ContentId,
    pub byte_length: u64,
    pub reference: ProjectFileRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAttachment {
    pub id: [u8; 32],
    pub content_id: ContentId,
    pub filename: String,
    pub mime_type: Option<String>,
    pub byte_length: u64,
    pub bytes: Vec<u8>,
    pub project_reference: Option<ProjectFileRef>,
}

/// Acceptance receipt for a source turn. The activity position is a derived
/// ordinal for first-accepted user/assistant turns, not a clock or version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnAcceptance<T> {
    pub value: T,
    pub inserted: bool,
    pub activity_position: Option<u64>,
    pub rel_turn_count: u64,
}

/// Source identity and derived position for one first-accepted Archive turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedTurnSource {
    pub conversation_id: String,
    pub node_id: String,
    pub activity_position: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalArchiveTurn {
    pub node: Node,
    pub content: Option<String>,
    pub attachments: Vec<SourceAttachment>,
    pub activity_position: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalArchiveBranch {
    pub archive_version: u64,
    pub branch: Branch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveHistorySnapshot {
    pub archive_version: u64,
    pub activity_cut: u64,
    pub turns: Vec<HistoricalArchiveTurn>,
    /// Every accepted Branch revision through the cut.
    pub branch_revisions: Vec<HistoricalArchiveBranch>,
    pub branches: Vec<Branch>,
    pub episodes: Vec<Episode>,
    pub import_receipts: Vec<ImportReceipt>,
    pub fragment_definitions: Vec<crate::FragmentDefinition>,
    pub conversation_tombstones: Vec<ConversationTombstone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTurn {
    pub node_id: String,
    pub role: String,
    pub principal_id: Option<String>,
    pub timestamp_ns: i64,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    pub id: String,
    pub conversation_id: String,
    pub leaf_node_id: String,
    pub canonical: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpisodeOrigin {
    Live,
    Import,
}

impl EpisodeOrigin {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Live => 1,
            Self::Import => 2,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Live),
            2 => Some(Self::Import),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpisodeBoundary {
    Size,
    Inactivity,
    CreateMemory,
    ImportEnd,
    Explicit,
}

impl EpisodeBoundary {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Size => 1,
            Self::Inactivity => 2,
            Self::CreateMemory => 3,
            Self::ImportEnd => 4,
            Self::Explicit => 5,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Size),
            2 => Some(Self::Inactivity),
            3 => Some(Self::CreateMemory),
            4 => Some(Self::ImportEnd),
            5 => Some(Self::Explicit),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Episode {
    pub id: EpisodeId,
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub origin: EpisodeOrigin,
    pub boundary: EpisodeBoundary,
    pub source_through_ns: i64,
    pub finalized_at_ns: i64,
}

impl Episode {
    pub(crate) fn new(
        conversation_id: String,
        start_node_id: String,
        end_node_id: String,
        origin: EpisodeOrigin,
        boundary: EpisodeBoundary,
        source_through_ns: i64,
        finalized_at_ns: i64,
    ) -> Self {
        let id = Self::stable_id(&conversation_id, &start_node_id, &end_node_id);
        Self {
            id,
            conversation_id,
            start_node_id,
            end_node_id,
            origin,
            boundary,
            source_through_ns,
            finalized_at_ns,
        }
    }

    pub(crate) fn stable_id(
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
    ) -> EpisodeId {
        let mut hash = Sha256::new();
        for value in [conversation_id, start_node_id, end_node_id] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        EpisodeId(hash.finalize().into())
    }
}

#[derive(Debug)]
pub enum ArchiveError {
    Container(ContainerError),
    WrongFileKind,
    WrongOwner,
    InvalidField(&'static str),
    FieldTooLarge,
    InvalidUtf8,
    CorruptRecord(&'static str),
    CorruptContent,
    HashCollision,
    MissingContent,
    InvalidAttachment,
    DuplicateAttachment,
    MissingNode,
    MissingParent,
    MissingBranch,
    ConflictingNode,
    InvalidBranchRevision,
    InvalidEpisodeRange,
    EvidenceLimitExceeded,
    ConflictingEpisode,
    ConflictingImportReceipt,
    ConflictingFragmentDefinition,
    ActivityPositionExhausted,
    ArchiveVersionExhausted,
    IndexCapacityExhausted,
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => write!(f, "Archive Container error: {error}"),
            Self::WrongFileKind => f.write_str("Archive requires a Reliquary Container"),
            Self::WrongOwner => f.write_str("Archive state belongs to another Container owner"),
            Self::InvalidField(field) => write!(f, "invalid Archive {field}"),
            Self::FieldTooLarge => f.write_str("Archive field exceeds the current format limit"),
            Self::InvalidUtf8 => f.write_str("Archive text field is not UTF-8"),
            Self::CorruptRecord(reason) => write!(f, "corrupt Archive record: {reason}"),
            Self::CorruptContent => f.write_str("Archive turn content digest does not match"),
            Self::HashCollision => f.write_str("Archive content hash collision"),
            Self::MissingContent => f.write_str("Archive turn content is unavailable"),
            Self::InvalidAttachment => f.write_str("Archive source attachment proof is invalid"),
            Self::DuplicateAttachment => f.write_str("Archive source turn repeats an attachment"),
            Self::MissingNode => f.write_str("Archive node does not exist"),
            Self::MissingParent => f.write_str("Archive turn parent does not exist"),
            Self::MissingBranch => f.write_str("Archive branch does not exist"),
            Self::ConflictingNode => f.write_str("Archive node identity has conflicting content"),
            Self::InvalidBranchRevision => {
                f.write_str("Archive branch revision is not a descendant")
            }
            Self::InvalidEpisodeRange => f.write_str("Archive Episode range is invalid"),
            Self::EvidenceLimitExceeded => {
                f.write_str("Archive evidence exceeds the requested bound")
            }
            Self::ConflictingEpisode => {
                f.write_str("Archive Episode identity has conflicting data")
            }
            Self::ConflictingImportReceipt => {
                f.write_str("Archive import identity has conflicting source mappings")
            }
            Self::ConflictingFragmentDefinition => {
                f.write_str("Archive fragment identity has conflicting source definition")
            }
            Self::ActivityPositionExhausted => {
                f.write_str("Archive accepted activity position exhausted")
            }
            Self::ArchiveVersionExhausted => f.write_str("Archive version exhausted"),
            Self::IndexCapacityExhausted => f.write_str("Archive index capacity exhausted"),
        }
    }
}

impl std::error::Error for ArchiveError {}
