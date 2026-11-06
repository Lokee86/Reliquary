use reliquary_container::{Container, ContainerError, ContainerIdentity};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const WORK_MAGIC: &[u8; 8] = b"RELINSW1";
const WORK_MAGIC_V2: &[u8; 8] = b"RELINSW2";
const RECEIPT_MAGIC: &[u8; 8] = b"RELINSR1";
pub const MAX_RECEIPT_IDS: usize = 100_000;
pub const MAX_RECEIPT_TEXT_BYTES: usize = 4096;
pub const MAX_HELD_PAYLOAD_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EpisodeId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InsomniaLeaseToken(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InsomniaPriority {
    ImmediateLive,
    Live,
    Import,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaWorkState {
    Pending,
    Processing,
    Complete,
    Failed,
    Terminal,
    Held,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMemoryRef {
    pub owner_id: String,
    pub memory_id: MemoryId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedEpisodeOrder {
    pub episode_id: EpisodeId,
    pub source_through_ns: i64,
    pub conversation_id: String,
    pub start_node_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaWork {
    pub episode_id: EpisodeId,
    pub priority: InsomniaPriority,
    pub state: InsomniaWorkState,
    pub attempt_count: u32,
    pub lease_owner: Option<String>,
    pub lease_token: Option<InsomniaLeaseToken>,
    pub lease_expires_ns: Option<i64>,
    pub retry_after_ns: Option<i64>,
    pub last_error: Option<String>,
    /// Exact bounded model result retained when automatic processing is held.
    pub held_payload: Option<Vec<u8>>,
    pub updated_at_ns: i64,
    pub revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCompletionState {
    pub episode_id: EpisodeId,
    pub attempt: u32,
    pub started_at_ns: i64,
    pub completed_at_ns: i64,
    pub extractor_model: String,
    pub extractor_version: String,
    pub memory_ids: Vec<MemoryId>,
    pub external_memory_refs: Vec<ExternalMemoryRef>,
    pub rejected_count: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionCheck {
    Apply,
    AlreadyAccepted,
}

#[derive(Debug)]
pub enum InsomniaError {
    Container(ContainerError),
    WrongOwner,
    UnacceptedEpisode,
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    MissingWork,
    InvalidLease,
    LeaseExpired,
    InvalidTransition,
    ReceiptConflict,
    VersionExhausted,
}
impl fmt::Display for InsomniaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(e) => e.fmt(f),
            Self::WrongOwner => f.write_str("Insomnia record belongs to another owner"),
            Self::UnacceptedEpisode => f.write_str("Episode was not admitted by the owner"),
            Self::CorruptRecord(s) => write!(f, "corrupt Insomnia record: {s}"),
            Self::InvalidField(s) => write!(f, "invalid Insomnia field: {s}"),
            Self::MissingWork => f.write_str("Insomnia work not found"),
            Self::InvalidLease => f.write_str("Insomnia lease is invalid"),
            Self::LeaseExpired => f.write_str("Insomnia lease expired"),
            Self::InvalidTransition => f.write_str("invalid Insomnia work transition"),
            Self::ReceiptConflict => {
                f.write_str("completion receipt conflicts with accepted evidence")
            }
            Self::VersionExhausted => f.write_str("Insomnia version exhausted"),
        }
    }
}
impl std::error::Error for InsomniaError {}
impl From<ContainerError> for InsomniaError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

pub struct InsomniaStore {
    owner: [u8; 16],
    identity: ContainerIdentity,
    accepted: BTreeMap<EpisodeId, AcceptedEpisodeOrder>,
    work: BTreeMap<EpisodeId, InsomniaWork>,
    /// Canonical chain binding every retained accepted work revision, not just
    /// the final projected status. Does not include grouped completion receipts.
    work_history_digests: BTreeMap<EpisodeId, [u8; 32]>,
    orders: BTreeMap<EpisodeId, AcceptedEpisodeOrder>,
    receipts: BTreeMap<(EpisodeId, u32), AcceptedCompletionState>,
}

mod completion;
mod reads;
mod work;

mod validate;
use validate::*;
mod record_codec;
use record_codec::*;
mod codec_primitives;
use codec_primitives::*;
