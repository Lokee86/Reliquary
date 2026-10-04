//! REL-owned durable provisional conversation journal, separate from Archive history
//! and independent of RuntimeHost. Archive effects are performed by the caller after
//! a durable FinalizeWon record and reconciled through exact stable-identity receipts.
use super::{HostCancelOutcome, HostOperationStatus, RuntimeCapability};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const FILE_MAGIC: &[u8; 8] = b"RQMUXJ01";
const SNAP_MAGIC: &[u8; 8] = b"RQMUXS01";
const HOST_PAYLOAD_MAGIC: &[u8; 8] = b"RQMUXH01";
const FRAME_LIMIT: usize = 16 * 1024 * 1024;
/// Staged binary payload bounds apply independently of the encoded journal frame limit.
pub(crate) const MAX_STAGED_ATTACHMENT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_STAGED_ATTACHMENT_COUNT: usize = 64;
pub(crate) const MAX_STAGED_SUBMISSION_BYTES: u64 = 256 * 1024 * 1024;
const JOURNAL_LIMIT: u64 = 1024 * 1024 * 1024;
const COMPACT_AT: u64 = 768 * 1024 * 1024;
const HASH_BYTES: usize = 32;
const HEADER_BYTES: u64 = 56;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(String);
impl ConversationId {
    pub fn new(v: impl Into<String>) -> Result<Self, JournalError> {
        let v = v.into();
        valid_id(&v, "conversation id")?;
        Ok(Self(v))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubmissionId(String);
impl SubmissionId {
    pub fn new(v: impl Into<String>) -> Result<Self, JournalError> {
        let v = v.into();
        valid_id(&v, "submission id")?;
        Ok(Self(v))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelOwnerId(Uuid);
impl RelOwnerId {
    pub fn from_uuid(v: Uuid) -> Self {
        Self(v)
    }
    pub fn parse_canonical(v: &str) -> Result<Self, JournalError> {
        let s = v
            .strip_prefix("rel-")
            .ok_or(JournalError::InvalidRelOwner)?;
        let id = Uuid::parse_str(s).map_err(|_| JournalError::InvalidRelOwner)?;
        if format!("rel-{id}") != v {
            return Err(JournalError::InvalidRelOwner);
        }
        Ok(Self(id))
    }
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
    fn filename(&self) -> String {
        format!("{}.rqj", self.0)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ConversationSubmissionId {
    pub conversation_id: ConversationId,
    pub submission_id: SubmissionId,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttachmentIdentity {
    pub attachment_id: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub content_hash: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Submission {
    pub id: ConversationSubmissionId,
    pub principal_id: String,
    pub scope_fingerprint: String,
    pub payload_hash: String,
    pub request_metadata: serde_json::Value,
    pub content: String,
    pub attachments: Vec<AttachmentIdentity>,
    /// Client time is metadata only and never participates in queue order.
    pub client_timestamp_ns: Option<i64>,
}
impl Submission {
    pub fn canonical_payload_hash(
        content: &str,
        attachments: &[AttachmentIdentity],
    ) -> Result<String, JournalError> {
        Ok(hex_digest(&serde_json::to_vec(&(content, attachments))?))
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FinalizeCapture {
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub committed_timestamp_ns: i64,
    pub verified_principal_id: String,
    pub policy_revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReceipt {
    pub node_id: String,
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub principal_id: String,
    pub committed_timestamp_ns: i64,
    pub payload_hash: String,
    pub attachment_identities: Vec<AttachmentIdentity>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PostCommitReceipt {
    pub operation_id: String,
    pub scheduler_receipt: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SubmissionState {
    Pending,
    Cancelled,
    Finalizing,
    Committed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DurableHostOperationPhase {
    HandoffIntent,
    Accepted,
    Pending,
    Running,
    CancelIntent,
    RunningCancellationRequested,
    Cancelled,
    Completed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableHostOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub phase: DurableHostOperationPhase,
    pub transition_revision: u64,
    #[serde(default)]
    pub cancel_requested: bool,
    /// Last authoritative non-Unknown host state, retained for conservative recovery.
    #[serde(default)]
    pub last_known_phase: Option<DurableHostOperationPhase>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmissionStatus {
    pub state: SubmissionState,
    pub admission_sequence: u64,
    pub verified_receive_ns: i64,
    pub tie_key: [u8; 16],
    pub transition_revision: u64,
    pub finalize_capture: Option<FinalizeCapture>,
    pub archive_receipt: Option<ArchiveReceipt>,
    pub post_commit_receipt: Option<PostCommitReceipt>,
    pub payload_retained: bool,
    #[serde(default)]
    pub host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnqueueOutcome {
    Durable(SubmissionStatus),
    Idempotent(SubmissionStatus),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitionOutcome {
    Durable(SubmissionStatus),
    Idempotent(SubmissionStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Projection {
    rel_id: RelOwnerId,
    last_sequence: u64,
    last_verified_timestamp_ns: i64,
    entries: BTreeMap<ConversationSubmissionId, Entry>,
    conversation_revisions: BTreeMap<ConversationId, u64>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Snapshot {
    rel_id: RelOwnerId,
    last_sequence: u64,
    last_verified_timestamp_ns: i64,
    entries: Vec<(ConversationSubmissionId, Entry)>,
    conversation_revisions: BTreeMap<ConversationId, u64>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Entry {
    submission: Submission,
    state: SubmissionState,
    admission_sequence: u64,
    verified_receive_ns: i64,
    tie_key: [u8; 16],
    transition_revision: u64,
    finalize_capture: Option<FinalizeCapture>,
    archive_receipt: Option<ArchiveReceipt>,
    post_commit_receipt: Option<PostCommitReceipt>,
    payload_retained: bool,
    #[serde(default)]
    host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
    request_fingerprint: String,
}
impl Projection {
    fn empty(id: RelOwnerId) -> Self {
        Self {
            rel_id: id,
            last_sequence: 0,
            last_verified_timestamp_ns: 0,
            entries: BTreeMap::new(),
            conversation_revisions: BTreeMap::new(),
        }
    }
    fn revision(&self, id: &ConversationId) -> u64 {
        *self.conversation_revisions.get(id).unwrap_or(&0)
    }
    fn status(&self, k: &ConversationSubmissionId) -> Option<SubmissionStatus> {
        self.entries.get(k).map(|e| SubmissionStatus {
            state: e.state,
            admission_sequence: e.admission_sequence,
            verified_receive_ns: e.verified_receive_ns,
            tie_key: e.tie_key,
            transition_revision: e.transition_revision,
            finalize_capture: e.finalize_capture.clone(),
            archive_receipt: e.archive_receipt.clone(),
            post_commit_receipt: e.post_commit_receipt.clone(),
            payload_retained: e.payload_retained,
            host_operations: e.host_operations.clone(),
        })
    }
    fn apply(&mut self, f: &Frame) -> Result<(), JournalError> {
        if f.sequence
            != self
                .last_sequence
                .checked_add(1)
                .ok_or(JournalError::SequenceOverflow)?
        {
            return Err(JournalError::Corrupt("sequence gap"));
        }
        let id = f.event.id().clone();
        match &f.event {
            Event::Enqueued {
                submission,
                verified_receive_ns,
                tie_key,
            } => {
                if self.entries.contains_key(&submission.id) {
                    return Err(JournalError::Corrupt("duplicate enqueue"));
                }
                if *verified_receive_ns < self.last_verified_timestamp_ns {
                    return Err(JournalError::Corrupt("timestamp regressed"));
                }
                self.last_verified_timestamp_ns = *verified_receive_ns;
                self.entries.insert(
                    submission.id.clone(),
                    Entry {
                        submission: submission.clone(),
                        state: SubmissionState::Pending,
                        admission_sequence: f.sequence,
                        verified_receive_ns: *verified_receive_ns,
                        tie_key: *tie_key,
                        transition_revision: 1,
                        finalize_capture: None,
                        archive_receipt: None,
                        post_commit_receipt: None,
                        payload_retained: true,
                        host_operations: BTreeMap::new(),
                        request_fingerprint: request_fingerprint(submission)?,
                    },
                );
            }
            Event::Cancelled {
                id,
                expected_revision,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(JournalError::Corrupt("stale cancel revision"));
                }
                let e = self
                    .entries
                    .get_mut(id)
                    .ok_or(JournalError::Corrupt("missing cancel target"))?;
                if e.state != SubmissionState::Pending {
                    return Err(JournalError::Corrupt("cancel lost state"));
                }
                if !e
                    .host_operations
                    .values()
                    .all(|operation| operation.phase == DurableHostOperationPhase::Cancelled)
                {
                    return Err(JournalError::Corrupt("cancel before host cancellation"));
                }
                e.state = SubmissionState::Cancelled;
                e.transition_revision += 1;
            }
            Event::FinalizeWon {
                id,
                expected_revision,
                capture,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(JournalError::Corrupt("stale finalize revision"));
                }
                let e = self
                    .entries
                    .get_mut(id)
                    .ok_or(JournalError::Corrupt("missing finalize target"))?;
                if e.state != SubmissionState::Pending
                    || capture.verified_principal_id != e.submission.principal_id
                    || !e
                        .host_operations
                        .values()
                        .all(|operation| operation.phase == DurableHostOperationPhase::Completed)
                {
                    return Err(JournalError::Corrupt("invalid finalize claim"));
                }
                e.state = SubmissionState::Finalizing;
                e.finalize_capture = Some(capture.clone());
                e.transition_revision += 1;
            }
            Event::ArchiveCommitted { id, receipt } => {
                let e = self
                    .entries
                    .get_mut(id)
                    .ok_or(JournalError::Corrupt("missing commit target"))?;
                if e.state != SubmissionState::Finalizing {
                    return Err(JournalError::Corrupt("commit before finalize"));
                }
                validate_receipt(e, receipt)?;
                e.state = SubmissionState::Committed;
                e.archive_receipt = Some(receipt.clone());
                e.transition_revision += 1;
            }
            Event::HostOperationUpdated {
                id,
                capability,
                operation,
            } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(JournalError::Corrupt("missing host operation target"))?;
                if entry.state != SubmissionState::Pending {
                    return Err(JournalError::Corrupt(
                        "host operation after native finalization",
                    ));
                }
                let previous = entry.host_operations.get(capability);
                if previous.is_some_and(|old| {
                    old.stable_id != operation.stable_id
                        || old.payload_digest != operation.payload_digest
                        || operation.transition_revision != old.transition_revision + 1
                        || (!operation.cancel_requested && old.cancel_requested)
                        || !host_phase_transition_allowed(&old.phase, &operation.phase)
                }) || previous.is_none()
                    && (operation.transition_revision != 1
                        || operation.phase != DurableHostOperationPhase::HandoffIntent
                        || operation.cancel_requested)
                {
                    return Err(JournalError::Corrupt("invalid host operation transition"));
                }
                entry.host_operations.insert(*capability, operation.clone());
            }
            Event::PostCommitScheduled { id, receipt } => {
                let e = self
                    .entries
                    .get_mut(id)
                    .ok_or(JournalError::Corrupt("missing postcommit target"))?;
                if e.state != SubmissionState::Committed
                    || e.archive_receipt.is_none()
                    || e.post_commit_receipt.is_some()
                {
                    return Err(JournalError::Corrupt("invalid postcommit receipt"));
                }
                e.post_commit_receipt = Some(receipt.clone());
                e.transition_revision += 1;
            }
        }
        if !matches!(&f.event, Event::HostOperationUpdated { .. }) {
            *self
                .conversation_revisions
                .entry(id.conversation_id)
                .or_default() += 1;
        }
        self.last_sequence = f.sequence;
        Ok(())
    }
    fn to_snapshot(&self) -> Snapshot {
        Snapshot {
            rel_id: self.rel_id,
            last_sequence: self.last_sequence,
            last_verified_timestamp_ns: self.last_verified_timestamp_ns,
            entries: self
                .entries
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            conversation_revisions: self.conversation_revisions.clone(),
        }
    }
    fn from_snapshot(s: Snapshot) -> Self {
        Self {
            rel_id: s.rel_id,
            last_sequence: s.last_sequence,
            last_verified_timestamp_ns: s.last_verified_timestamp_ns,
            entries: s.entries.into_iter().collect(),
            conversation_revisions: s.conversation_revisions,
        }
    }
    fn compacted(&self) -> Self {
        let mut n = self.clone();
        for e in n.entries.values_mut() {
            let host_stages_terminal = e.host_operations.values().all(|operation| {
                matches!(
                    operation.phase,
                    DurableHostOperationPhase::Cancelled | DurableHostOperationPhase::Completed
                )
            });
            if host_stages_terminal
                && (e.state == SubmissionState::Cancelled
                    || (e.state == SubmissionState::Committed && e.post_commit_receipt.is_some()))
            {
                e.payload_retained = false;
                e.submission.content.clear();
                e.submission.attachments.clear()
            }
        }
        n
    }
}
fn host_phase_transition_allowed(
    old: &DurableHostOperationPhase,
    new: &DurableHostOperationPhase,
) -> bool {
    use DurableHostOperationPhase::*;
    match old {
        HandoffIntent => matches!(
            new,
            Accepted | Pending | Running | CancelIntent | Cancelled | Completed | Unknown
        ),
        Accepted => matches!(
            new,
            Pending | Running | CancelIntent | Cancelled | Completed | Unknown
        ),
        Pending => matches!(
            new,
            Running | CancelIntent | Cancelled | Completed | Unknown
        ),
        Running => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Completed | Unknown
        ),
        CancelIntent => matches!(
            new,
            Pending | Running | RunningCancellationRequested | Cancelled | Completed | Unknown
        ),
        RunningCancellationRequested => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Cancelled | Completed | Unknown
        ),
        Unknown => matches!(
            new,
            Accepted
                | Pending
                | Running
                | CancelIntent
                | RunningCancellationRequested
                | Cancelled
                | Completed
        ),
        Cancelled | Completed => false,
    }
}

fn validate_receipt(e: &Entry, r: &ArchiveReceipt) -> Result<(), JournalError> {
    let c = e
        .finalize_capture
        .as_ref()
        .ok_or(JournalError::Corrupt("missing captured finalize"))?;
    if r.node_id != e.submission.id.submission_id.as_str()
        || r.branch_id != c.branch_id
        || r.parent_node_id != c.parent_node_id
        || r.principal_id != c.verified_principal_id
        || r.committed_timestamp_ns != c.committed_timestamp_ns
        || r.payload_hash != e.submission.payload_hash
        || r.attachment_identities != e.submission.attachments
    {
        return Err(JournalError::ArchiveIdentityConflict);
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Frame {
    sequence: u64,
    event: Event,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Event {
    Enqueued {
        submission: Submission,
        verified_receive_ns: i64,
        tie_key: [u8; 16],
    },
    Cancelled {
        id: ConversationSubmissionId,
        expected_revision: u64,
    },
    FinalizeWon {
        id: ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    },
    ArchiveCommitted {
        id: ConversationSubmissionId,
        receipt: ArchiveReceipt,
    },
    PostCommitScheduled {
        id: ConversationSubmissionId,
        receipt: PostCommitReceipt,
    },
    HostOperationUpdated {
        id: ConversationSubmissionId,
        capability: RuntimeCapability,
        operation: DurableHostOperation,
    },
}
impl Event {
    fn id(&self) -> &ConversationSubmissionId {
        match self {
            Self::Enqueued { submission, .. } => &submission.id,
            Self::Cancelled { id, .. }
            | Self::FinalizeWon { id, .. }
            | Self::ArchiveCommitted { id, .. }
            | Self::PostCommitScheduled { id, .. }
            | Self::HostOperationUpdated { id, .. } => id,
        }
    }
}

#[derive(Debug)]
pub enum JournalError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    InvalidRelOwner,
    InvalidIdentifier(&'static str),
    DuplicatePayloadConflict,
    MissingStagedAttachment,
    PayloadHashMismatch,
    MissingSubmission,
    InvalidState,
    StaleRevision { expected: u64, actual: u64 },
    ArchiveIdentityConflict,
    SequenceOverflow,
    JournalFull,
    RecoveryRequired,
    Corrupt(&'static str),
    UnsupportedDurability,
    HostOperationConflict,
}
impl From<std::io::Error> for JournalError {
    fn from(v: std::io::Error) -> Self {
        Self::Io(v)
    }
}
impl From<serde_json::Error> for JournalError {
    fn from(v: serde_json::Error) -> Self {
        Self::Serialization(v)
    }
}
impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "journal I/O error: {e}"),
            Self::Serialization(e) => write!(f, "journal serialization error: {e}"),
            Self::InvalidRelOwner => f.write_str("invalid canonical REL ID"),
            Self::InvalidIdentifier(k) => write!(f, "invalid {k}"),
            Self::DuplicatePayloadConflict => {
                f.write_str("composite key reused with different request")
            }
            Self::MissingStagedAttachment => f.write_str("staged attachment is missing"),
            Self::PayloadHashMismatch => {
                f.write_str("payload hash does not match message and staged attachments")
            }
            Self::MissingSubmission => f.write_str("submission not found"),
            Self::InvalidState => f.write_str("submission in invalid state"),
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: expected {expected}, actual {actual}")
            }
            Self::ArchiveIdentityConflict => {
                f.write_str("Archive receipt does not exactly match captured turn")
            }
            Self::SequenceOverflow => f.write_str("journal sequence overflow"),
            Self::JournalFull => f.write_str("REL journal hard size limit reached"),
            Self::RecoveryRequired => f.write_str("journal fenced pending recovery"),
            Self::Corrupt(m) => write!(f, "journal corruption: {m}"),
            Self::UnsupportedDurability => {
                f.write_str("required filesystem durability unavailable")
            }
            Self::HostOperationConflict => {
                f.write_str("host operation identity or transition conflicts with durable intent")
            }
        }
    }
}
impl std::error::Error for JournalError {}

pub struct RelJournal {
    rel_id: RelOwnerId,
    path: PathBuf,
    file: File,
    projection: Projection,
    failed: bool,
    _writer_lock: File,
}
impl RelJournal {
    /// Opens the per-REL file under state_dir/conversation-queues and validates its owner UUID.
    pub fn open(state_dir: impl AsRef<Path>, rel_id: RelOwnerId) -> Result<Self, JournalError> {
        let state_dir = state_dir.as_ref();
        let dir = state_dir.join("conversation-queues");
        fs::create_dir_all(&dir)?;
        sync_dir(state_dir)?;
        let path = dir.join(rel_id.filename());
        let lock_path = path.with_extension("rqj.lock");
        let writer_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(lock_path)?;
        writer_lock
            .try_lock()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::WouldBlock, e.to_string()))?;
        let existed = path.exists();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        if !existed {
            file.write_all(&owner_header(FILE_MAGIC, rel_id))?;
            file.sync_all()?;
        }
        let (projection, valid) = recover(&mut file, rel_id)?;
        let actual = file.metadata()?.len();
        if valid < actual {
            file.set_len(valid)?;
            file.sync_all()?
        }
        sync_dir(&dir)?;
        file.seek(SeekFrom::End(0))?;
        let journal = Self {
            rel_id,
            path,
            file,
            projection,
            failed: false,
            _writer_lock: writer_lock,
        };
        journal.collect_unreferenced_attachments()?;
        Ok(journal)
    }
    pub fn rel_id(&self) -> RelOwnerId {
        self.rel_id
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn sequence(&self) -> u64 {
        self.projection.last_sequence
    }
    pub fn conversation_revision(&self, id: &ConversationId) -> u64 {
        self.projection.revision(id)
    }
    /// Returns the durable initiating principal without exposing compacted payload data.
    /// The principal remains retained in cancellation and terminal identity tombstones.
    pub fn submitter_principal_id(&self, id: &ConversationSubmissionId) -> Option<&str> {
        self.projection
            .entries
            .get(id)
            .map(|entry| entry.submission.principal_id.as_str())
    }

    pub fn status(&self, id: &ConversationSubmissionId) -> Option<SubmissionStatus> {
        self.projection.status(id)
    }
    pub fn submission(&self, id: &ConversationSubmissionId) -> Option<&Submission> {
        self.projection
            .entries
            .get(id)
            .filter(|e| e.payload_retained)
            .map(|e| &e.submission)
    }
    pub fn statuses(&self) -> Vec<(ConversationSubmissionId, SubmissionStatus)> {
        self.projection
            .entries
            .keys()
            .filter_map(|k| self.status(k).map(|v| (k.clone(), v)))
            .collect()
    }

    pub fn host_operation(
        &self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Option<DurableHostOperation> {
        self.projection
            .entries
            .get(id)?
            .host_operations
            .get(&capability)
            .cloned()
    }

    pub fn host_operations(
        &self,
    ) -> Vec<(
        ConversationSubmissionId,
        RuntimeCapability,
        DurableHostOperation,
    )> {
        self.projection
            .entries
            .iter()
            .flat_map(|(id, entry)| {
                entry
                    .host_operations
                    .iter()
                    .map(|(capability, operation)| (id.clone(), *capability, operation.clone()))
            })
            .collect()
    }

    pub fn begin_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
    ) -> Result<TransitionOutcome, JournalError> {
        valid_id(&stable_id, "host operation id")?;
        self.set_host_operation_phase(
            id,
            capability,
            stable_id,
            payload_digest,
            DurableHostOperationPhase::HandoffIntent,
        )
    }

    pub fn record_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        receipt: &super::HandoffReceipt,
    ) -> Result<TransitionOutcome, JournalError> {
        if !receipt.durably_accepted {
            return Err(JournalError::HostOperationConflict);
        }
        let current = self
            .host_operation(id, capability)
            .ok_or(JournalError::MissingSubmission)?;
        if current.stable_id != receipt.stable_id
            || current.payload_digest != receipt.payload_digest
        {
            return Err(JournalError::HostOperationConflict);
        }
        // A late acknowledgment cannot erase a cancel intent or a status already
        // observed while the handoff call was in flight.
        if current.cancel_requested
            || matches!(
                current.phase,
                DurableHostOperationPhase::Cancelled
                    | DurableHostOperationPhase::Completed
                    | DurableHostOperationPhase::Pending
                    | DurableHostOperationPhase::Running
                    | DurableHostOperationPhase::RunningCancellationRequested
            )
        {
            return Ok(TransitionOutcome::Idempotent(
                self.projection.status(id).unwrap(),
            ));
        }
        self.set_host_operation_phase(
            id,
            capability,
            receipt.stable_id.clone(),
            receipt.payload_digest,
            DurableHostOperationPhase::Accepted,
        )
    }

    pub fn record_host_status(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        status: HostOperationStatus,
    ) -> Result<TransitionOutcome, JournalError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(JournalError::MissingSubmission)?;
        let phase = match status {
            HostOperationStatus::Pending if current.cancel_requested => {
                DurableHostOperationPhase::CancelIntent
            }
            HostOperationStatus::Pending => DurableHostOperationPhase::Pending,
            HostOperationStatus::Running if current.cancel_requested => {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostOperationStatus::Running => DurableHostOperationPhase::Running,
            HostOperationStatus::Completed => DurableHostOperationPhase::Completed,
            HostOperationStatus::Cancelled => DurableHostOperationPhase::Cancelled,
            HostOperationStatus::Unknown => DurableHostOperationPhase::Unknown,
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )
    }

    pub fn begin_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Result<TransitionOutcome, JournalError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(JournalError::MissingSubmission)?;
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            DurableHostOperationPhase::CancelIntent,
        )
    }

    pub fn record_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        outcome: HostCancelOutcome,
    ) -> Result<TransitionOutcome, JournalError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(JournalError::MissingSubmission)?;
        let phase = match outcome {
            HostCancelOutcome::CancelledBeforeStart
            | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Cancelled) => {
                DurableHostOperationPhase::Cancelled
            }
            HostCancelOutcome::RunningCancellationRequested => {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Completed) => {
                DurableHostOperationPhase::Completed
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Pending)
                if current.cancel_requested =>
            {
                DurableHostOperationPhase::CancelIntent
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Pending) => {
                DurableHostOperationPhase::Pending
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Running)
                if current.cancel_requested =>
            {
                DurableHostOperationPhase::RunningCancellationRequested
            }
            HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Running) => {
                DurableHostOperationPhase::Running
            }
            HostCancelOutcome::Unknown
            | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Unknown) => {
                DurableHostOperationPhase::Unknown
            }
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )
    }

    fn set_host_operation_phase(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
        phase: DurableHostOperationPhase,
    ) -> Result<TransitionOutcome, JournalError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if entry.state != SubmissionState::Pending {
            return Err(JournalError::InvalidState);
        }
        let previous = entry.host_operations.get(&capability);
        let revision = match previous {
            Some(old) => {
                if old.stable_id != stable_id || old.payload_digest != payload_digest {
                    return Err(JournalError::HostOperationConflict);
                }
                if old.phase == phase {
                    return Ok(TransitionOutcome::Idempotent(
                        self.projection.status(id).unwrap(),
                    ));
                }
                if !host_phase_transition_allowed(&old.phase, &phase) {
                    // Remote status calls may complete out of order. Keep the
                    // stronger durable state when an older Pending/Running response arrives.
                    if old.phase != DurableHostOperationPhase::Unknown
                        && matches!(
                            phase,
                            DurableHostOperationPhase::Pending | DurableHostOperationPhase::Running
                        )
                        && host_phase_transition_allowed(&phase, &old.phase)
                    {
                        return Ok(TransitionOutcome::Idempotent(
                            self.projection.status(id).unwrap(),
                        ));
                    }
                    return Err(JournalError::InvalidState);
                }
                old.transition_revision
                    .checked_add(1)
                    .ok_or(JournalError::SequenceOverflow)?
            }
            None => {
                if phase != DurableHostOperationPhase::HandoffIntent {
                    return Err(JournalError::InvalidState);
                }
                1
            }
        };
        let cancel_requested = previous.is_some_and(|old| old.cancel_requested)
            || phase == DurableHostOperationPhase::CancelIntent
            || phase == DurableHostOperationPhase::RunningCancellationRequested;
        let last_known_phase = match &phase {
            DurableHostOperationPhase::Unknown => previous.map(|old| {
                old.last_known_phase
                    .clone()
                    .unwrap_or_else(|| old.phase.clone())
            }),
            _ => Some(phase.clone()),
        };
        let operation = DurableHostOperation {
            stable_id,
            payload_digest,
            phase,
            transition_revision: revision,
            cancel_requested,
            last_known_phase,
        };
        self.append(Event::HostOperationUpdated {
            id: id.clone(),
            capability,
            operation,
        })?;
        Ok(TransitionOutcome::Durable(
            self.projection.status(id).unwrap(),
        ))
    }
    pub fn pending_order(&self, c: &ConversationId) -> Vec<ConversationSubmissionId> {
        let mut v: Vec<_> = self
            .projection
            .entries
            .iter()
            .filter(|(k, e)| &k.conversation_id == c && e.state == SubmissionState::Pending)
            .map(|(k, e)| {
                (
                    e.verified_receive_ns,
                    e.tie_key,
                    e.admission_sequence,
                    k.clone(),
                )
            })
            .collect();
        v.sort_by_key(|x| (x.0, x.1, x.2));
        v.into_iter().map(|x| x.3).collect()
    }
    /// Validate per-submission binary payload bounds before creating durable spool files.
    pub(crate) fn validate_staged_payload_sizes(
        sizes: impl IntoIterator<Item = usize>,
    ) -> Result<(), JournalError> {
        let mut count = 0usize;
        let mut total = 0u64;
        for size in sizes {
            count = count.checked_add(1).ok_or(JournalError::JournalFull)?;
            if count > MAX_STAGED_ATTACHMENT_COUNT || size > MAX_STAGED_ATTACHMENT_BYTES {
                return Err(JournalError::JournalFull);
            }
            total = total
                .checked_add(u64::try_from(size).map_err(|_| JournalError::JournalFull)?)
                .ok_or(JournalError::JournalFull)?;
            if total > MAX_STAGED_SUBMISSION_BYTES {
                return Err(JournalError::JournalFull);
            }
        }
        Ok(())
    }

    fn staging_dir(&self) -> Result<PathBuf, JournalError> {
        Ok(self
            .path
            .parent()
            .ok_or(JournalError::UnsupportedDurability)?
            .join("staging")
            .join(self.rel_id.0.to_string()))
    }

    /// Stage a complete, bounded submission batch; size/count errors occur before any writes.
    pub(crate) fn stage_attachments<'a>(
        &mut self,
        attachments: impl IntoIterator<Item = (&'a str, &'a str, Option<&'a str>, &'a [u8])>,
    ) -> Result<Vec<AttachmentIdentity>, JournalError> {
        let attachments: Vec<_> = attachments.into_iter().collect();
        Self::validate_staged_payload_sizes(
            attachments.iter().map(|(_, _, _, bytes)| bytes.len()),
        )?;
        attachments
            .into_iter()
            .map(|(id, filename, mime_type, bytes)| {
                self.stage_attachment(id, filename, mime_type.map(str::to_owned), bytes)
            })
            .collect()
    }

    pub fn stage_attachment(
        &mut self,
        attachment_id: impl Into<String>,
        filename: impl Into<String>,
        mime_type: Option<String>,
        bytes: &[u8],
    ) -> Result<AttachmentIdentity, JournalError> {
        self.ensure()?;
        if bytes.len() > MAX_STAGED_ATTACHMENT_BYTES {
            return Err(JournalError::JournalFull);
        }
        let attachment_id = attachment_id.into();
        let filename = filename.into();
        valid_id(&attachment_id, "attachment id")?;
        valid_id(&filename, "attachment filename")?;
        if let Some(mime) = &mime_type {
            valid_id(mime, "attachment MIME type")?;
        }
        let content_hash = hex_digest(bytes);
        let queue_dir = self
            .path
            .parent()
            .ok_or(JournalError::UnsupportedDurability)?;
        let staging_root = queue_dir.join("staging");
        let root_existed = staging_root.exists();
        fs::create_dir_all(&staging_root)?;
        if !root_existed {
            sync_dir(queue_dir)?;
        }
        let dir = self.staging_dir()?;
        let existed = dir.exists();
        fs::create_dir_all(&dir)?;
        if !existed {
            sync_dir(&staging_root)?;
        }
        let final_path = dir.join(format!("{content_hash}.blob"));
        if final_path.exists() {
            let existing = fs::read(&final_path)?;
            if hex_digest(&existing) != content_hash {
                return Err(JournalError::Corrupt("staged attachment hash mismatch"));
            }
        } else {
            let temp = dir.join(format!(".{content_hash}-{}.tmp", Uuid::new_v4()));
            let mut staged = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            staged.write_all(bytes)?;
            staged.sync_all()?;
            drop(staged);
            if let Err(error) = replace(&temp, &final_path) {
                let _ = fs::remove_file(&temp);
                if !final_path.exists() {
                    return Err(error);
                }
            }
            sync_dir(&dir)?;
            let verify = fs::read(&final_path)?;
            if hex_digest(&verify) != content_hash {
                return Err(JournalError::Corrupt(
                    "staged attachment verification failed",
                ));
            }
        }
        sync_dir(&dir)?;
        Ok(AttachmentIdentity {
            attachment_id,
            filename,
            mime_type,
            content_hash,
        })
    }
    pub fn read_attachment(&self, identity: &AttachmentIdentity) -> Result<Vec<u8>, JournalError> {
        validate_content_hash(&identity.content_hash)?;
        let dir = self.staging_dir()?;
        let bytes = fs::read(dir.join(format!("{}.blob", identity.content_hash)))
            .map_err(|_| JournalError::MissingStagedAttachment)?;
        if hex_digest(&bytes) != identity.content_hash {
            return Err(JournalError::Corrupt("staged attachment hash mismatch"));
        }
        Ok(bytes)
    }

    /// Build the bounded, compact host wire payload for a retained submission.
    /// Encoding: `RQMUXH01`, metadata length (u64 LE), canonical JSON `(owner_id,
    /// Submission)`, then each attachment in submission order as `u64 LE length`
    /// followed by raw bytes. The returned digest covers every encoded byte.
    pub(crate) fn host_payload(
        &self,
        owner_id: &str,
        id: &ConversationSubmissionId,
    ) -> Result<(Vec<u8>, [u8; 32]), JournalError> {
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if !entry.payload_retained {
            return Err(JournalError::MissingSubmission);
        }
        let metadata = serde_json::to_vec(&(owner_id, &entry.submission))?;
        if metadata.len() > FRAME_LIMIT {
            return Err(JournalError::JournalFull);
        }
        let staging_dir = self.staging_dir()?;
        let attachment_sizes = entry
            .submission
            .attachments
            .iter()
            .map(|identity| {
                validate_content_hash(&identity.content_hash)?;
                let size =
                    fs::metadata(staging_dir.join(format!("{}.blob", identity.content_hash)))
                        .map_err(|_| JournalError::MissingStagedAttachment)?
                        .len();
                usize::try_from(size).map_err(|_| JournalError::JournalFull)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::validate_staged_payload_sizes(attachment_sizes.iter().copied())?;
        let framing_bytes = 8usize
            .checked_add(8)
            .and_then(|n| n.checked_add(metadata.len()))
            .and_then(|n| n.checked_add(attachment_sizes.len().checked_mul(8)?))
            .ok_or(JournalError::JournalFull)?;
        let payload_bytes = attachment_sizes
            .iter()
            .try_fold(framing_bytes, |total, size| {
                total.checked_add(*size).ok_or(JournalError::JournalFull)
            })?;
        let max_bytes = usize::try_from(MAX_STAGED_SUBMISSION_BYTES)
            .map_err(|_| JournalError::JournalFull)?
            .checked_add(FRAME_LIMIT)
            .and_then(|n| n.checked_add((MAX_STAGED_ATTACHMENT_COUNT + 1) * 8))
            .ok_or(JournalError::JournalFull)?;
        if payload_bytes > max_bytes {
            return Err(JournalError::JournalFull);
        }
        let mut payload = Vec::with_capacity(payload_bytes);
        payload.extend_from_slice(HOST_PAYLOAD_MAGIC);
        payload.extend_from_slice(
            &u64::try_from(metadata.len())
                .map_err(|_| JournalError::JournalFull)?
                .to_le_bytes(),
        );
        payload.extend_from_slice(&metadata);
        for (identity, expected_size) in entry.submission.attachments.iter().zip(attachment_sizes) {
            let bytes = self.read_attachment(identity)?;
            if bytes.len() != expected_size {
                return Err(JournalError::Corrupt("staged attachment size changed"));
            }
            payload.extend_from_slice(
                &u64::try_from(bytes.len())
                    .map_err(|_| JournalError::JournalFull)?
                    .to_le_bytes(),
            );
            payload.extend_from_slice(&bytes);
        }
        let digest = Sha256::digest(&payload).into();
        Ok((payload, digest))
    }
    fn verify_staged_attachments(
        &self,
        attachments: &[AttachmentIdentity],
    ) -> Result<(), JournalError> {
        let mut seen = std::collections::BTreeSet::new();
        for a in attachments {
            valid_id(&a.attachment_id, "attachment id")?;
            valid_id(&a.filename, "attachment filename")?;
            if let Some(mime) = &a.mime_type {
                valid_id(mime, "attachment MIME type")?;
            }
            if !seen.insert(a.attachment_id.as_str()) {
                return Err(JournalError::InvalidIdentifier("duplicate attachment id"));
            }
            self.read_attachment(a)?;
        }
        Ok(())
    }
    /// Check the exact encoded Enqueued frame before staged payloads are written.
    pub(crate) fn preflight_enqueue_frame(
        &self,
        submission: &Submission,
        wall_ns: i64,
        expected_revision: u64,
        tie_key: [u8; 16],
    ) -> Result<bool, JournalError> {
        self.ensure()?;
        valid_id(&submission.principal_id, "principal id")?;
        valid_id(&submission.scope_fingerprint, "scope fingerprint")?;
        valid_id(&submission.payload_hash, "payload hash")?;
        let expected_hash =
            Submission::canonical_payload_hash(&submission.content, &submission.attachments)?;
        if submission.payload_hash != expected_hash {
            return Err(JournalError::PayloadHashMismatch);
        }
        let mut seen = std::collections::BTreeSet::new();
        for attachment in &submission.attachments {
            valid_id(&attachment.attachment_id, "attachment id")?;
            valid_id(&attachment.filename, "attachment filename")?;
            if let Some(mime) = &attachment.mime_type {
                valid_id(mime, "attachment MIME type")?;
            }
            validate_content_hash(&attachment.content_hash)?;
            if !seen.insert(attachment.attachment_id.as_str()) {
                return Err(JournalError::InvalidIdentifier("duplicate attachment id"));
            }
        }
        if let Some(entry) = self.projection.entries.get(&submission.id) {
            if !same_request(entry, submission)? {
                return Err(JournalError::DuplicatePayloadConflict);
            }
            return Ok(true);
        }
        let actual_revision = self.conversation_revision(&submission.id.conversation_id);
        if actual_revision != expected_revision {
            return Err(JournalError::StaleRevision {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
        let sequence = self
            .projection
            .last_sequence
            .checked_add(1)
            .ok_or(JournalError::SequenceOverflow)?;
        let frame = Frame {
            sequence,
            event: Event::Enqueued {
                submission: submission.clone(),
                verified_receive_ns: wall_ns.max(self.projection.last_verified_timestamp_ns),
                tie_key,
            },
        };
        if serde_json::to_vec(&frame)?.len() > FRAME_LIMIT {
            return Err(JournalError::JournalFull);
        }
        Ok(false)
    }
    pub(crate) fn enqueue_with_tie_key(
        &mut self,
        s: Submission,
        wall_ns: i64,
        expected_revision: u64,
        tie_key: [u8; 16],
    ) -> Result<EnqueueOutcome, JournalError> {
        self.ensure()?;
        valid_id(&s.principal_id, "principal id")?;
        valid_id(&s.scope_fingerprint, "scope fingerprint")?;
        valid_id(&s.payload_hash, "payload hash")?;
        let expected_hash = Submission::canonical_payload_hash(&s.content, &s.attachments)?;
        if s.payload_hash != expected_hash {
            return Err(JournalError::PayloadHashMismatch);
        }
        if let Some(e) = self.projection.entries.get(&s.id) {
            if !same_request(e, &s)? {
                return Err(JournalError::DuplicatePayloadConflict);
            }
            return Ok(EnqueueOutcome::Idempotent(
                self.projection.status(&s.id).unwrap(),
            ));
        }
        self.verify_staged_attachments(&s.attachments)?;
        let actual = self.conversation_revision(&s.id.conversation_id);
        if actual != expected_revision {
            return Err(JournalError::StaleRevision {
                expected: expected_revision,
                actual,
            });
        }
        let id = s.id.clone();
        let time = wall_ns.max(self.projection.last_verified_timestamp_ns);
        let _ = self.preflight_enqueue_frame(&s, time, expected_revision, tie_key)?;
        self.append(Event::Enqueued {
            submission: s,
            verified_receive_ns: time,
            tie_key,
        })?;
        Ok(EnqueueOutcome::Durable(
            self.projection.status(&id).unwrap(),
        ))
    }
    pub fn enqueue(
        &mut self,
        s: Submission,
        wall_ns: i64,
        expected_revision: u64,
    ) -> Result<EnqueueOutcome, JournalError> {
        self.enqueue_with_tie_key(s, wall_ns, expected_revision, *Uuid::new_v4().as_bytes())
    }
    pub fn cancel(
        &mut self,
        id: &ConversationSubmissionId,
        expected: u64,
    ) -> Result<TransitionOutcome, JournalError> {
        self.ensure()?;
        let e = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if e.state == SubmissionState::Cancelled {
            return Ok(TransitionOutcome::Idempotent(
                self.projection.status(id).unwrap(),
            ));
        }
        if e.state != SubmissionState::Pending {
            return Err(JournalError::InvalidState);
        }
        if !e
            .host_operations
            .values()
            .all(|operation| operation.phase == DurableHostOperationPhase::Cancelled)
        {
            return Err(JournalError::InvalidState);
        }
        let actual = self.conversation_revision(&id.conversation_id);
        if actual != expected {
            return Err(JournalError::StaleRevision { expected, actual });
        }
        self.append(Event::Cancelled {
            id: id.clone(),
            expected_revision: expected,
        })?;
        Ok(TransitionOutcome::Durable(
            self.projection.status(id).unwrap(),
        ))
    }
    pub fn claim_finalize(
        &mut self,
        id: &ConversationSubmissionId,
        expected: u64,
        c: FinalizeCapture,
    ) -> Result<TransitionOutcome, JournalError> {
        self.ensure()?;
        let e = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if e.state == SubmissionState::Finalizing {
            if e.finalize_capture.as_ref() == Some(&c) {
                return Ok(TransitionOutcome::Idempotent(
                    self.projection.status(id).unwrap(),
                ));
            }
            return Err(JournalError::ArchiveIdentityConflict);
        }
        if e.state != SubmissionState::Pending {
            return Err(JournalError::InvalidState);
        }
        if !e
            .host_operations
            .values()
            .all(|operation| operation.phase == DurableHostOperationPhase::Completed)
        {
            return Err(JournalError::InvalidState);
        }
        let actual = self.conversation_revision(&id.conversation_id);
        if actual != expected {
            return Err(JournalError::StaleRevision { expected, actual });
        }
        if c.verified_principal_id != e.submission.principal_id {
            return Err(JournalError::ArchiveIdentityConflict);
        }
        self.append(Event::FinalizeWon {
            id: id.clone(),
            expected_revision: expected,
            capture: c,
        })?;
        Ok(TransitionOutcome::Durable(
            self.projection.status(id).unwrap(),
        ))
    }
    pub fn record_archive_commit(
        &mut self,
        id: &ConversationSubmissionId,
        r: ArchiveReceipt,
    ) -> Result<TransitionOutcome, JournalError> {
        self.ensure()?;
        let e = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if e.state == SubmissionState::Committed {
            if e.archive_receipt.as_ref() == Some(&r) {
                return Ok(TransitionOutcome::Idempotent(
                    self.projection.status(id).unwrap(),
                ));
            }
            return Err(JournalError::ArchiveIdentityConflict);
        }
        if e.state != SubmissionState::Finalizing {
            return Err(JournalError::InvalidState);
        }
        validate_receipt(e, &r)?;
        self.append(Event::ArchiveCommitted {
            id: id.clone(),
            receipt: r,
        })?;
        Ok(TransitionOutcome::Durable(
            self.projection.status(id).unwrap(),
        ))
    }
    pub fn record_postcommit_receipt(
        &mut self,
        id: &ConversationSubmissionId,
        r: PostCommitReceipt,
    ) -> Result<TransitionOutcome, JournalError> {
        self.ensure()?;
        let e = self
            .projection
            .entries
            .get(id)
            .ok_or(JournalError::MissingSubmission)?;
        if e.post_commit_receipt.as_ref() == Some(&r) {
            return Ok(TransitionOutcome::Idempotent(
                self.projection.status(id).unwrap(),
            ));
        }
        if e.state != SubmissionState::Committed || e.post_commit_receipt.is_some() {
            return Err(JournalError::InvalidState);
        }
        self.append(Event::PostCommitScheduled {
            id: id.clone(),
            receipt: r,
        })?;
        Ok(TransitionOutcome::Durable(
            self.projection.status(id).unwrap(),
        ))
    }
    pub fn reconcile_archive_node(
        &mut self,
        id: &ConversationSubmissionId,
        r: ArchiveReceipt,
    ) -> Result<TransitionOutcome, JournalError> {
        self.record_archive_commit(id, r)
    }
    pub fn needs_compaction(&self) -> Result<bool, JournalError> {
        self.needs_compaction_at(COMPACT_AT)
    }
    pub(super) fn needs_compaction_at(&self, threshold: u64) -> Result<bool, JournalError> {
        let snapshot_size = serde_json::to_vec(&self.projection.to_snapshot())?.len() as u64
            + HEADER_BYTES
            + 8
            + HASH_BYTES as u64;
        Ok(self.file.metadata()?.len() >= threshold || snapshot_size >= threshold)
    }
    /// Compacts every conversation together; large snapshots use a 64-bit length and may exceed a frame.
    pub fn compact(&mut self) -> Result<(), JournalError> {
        self.ensure()?;
        let candidate = self.projection.compacted();
        let payload = serde_json::to_vec(&candidate.to_snapshot())?;
        if payload.len() as u64 + HEADER_BYTES + 8 + HASH_BYTES as u64 > JOURNAL_LIMIT {
            return Err(JournalError::JournalFull);
        }
        let tmp = self.path.with_extension("rqj.compacting");
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(&owner_header(SNAP_MAGIC, self.rel_id))?;
        f.write_all(&(payload.len() as u64).to_le_bytes())?;
        f.write_all(&payload)?;
        f.write_all(&hash(&payload))?;
        f.sync_all()?;
        drop(f);
        self.failed = true;
        let parked = File::open(self.path.with_extension("rqj.lock"))?;
        let old_handle = std::mem::replace(&mut self.file, parked);
        drop(old_handle);
        replace(&tmp, &self.path)?;
        sync_dir(
            self.path
                .parent()
                .ok_or(JournalError::UnsupportedDurability)?,
        )?;
        self.file = OpenOptions::new().read(true).write(true).open(&self.path)?;
        let (installed, valid) = recover(&mut self.file, self.rel_id)?;
        if valid != self.file.metadata()?.len() || installed != candidate {
            self.failed = true;
            return Err(JournalError::Corrupt("snapshot verification failed"));
        }
        self.file.seek(SeekFrom::End(0))?;
        self.projection = installed;
        self.failed = false;
        self.collect_unreferenced_attachments()?;
        Ok(())
    }
    /// Remove only content-addressed spool blobs absent from every retained payload.
    /// Call under this journal's writer lock after recovery or a durable compaction.
    pub(crate) fn collect_unreferenced_attachments(&self) -> Result<(), JournalError> {
        let dir = self.staging_dir()?;
        if !dir.exists() {
            return Ok(());
        }
        let referenced = self
            .projection
            .entries
            .values()
            .filter(|entry| entry.payload_retained)
            .flat_map(|entry| entry.submission.attachments.iter())
            .map(|identity| identity.content_hash.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut removed_any = false;
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(content_hash) = name.strip_suffix(".blob") else {
                continue;
            };
            if validate_content_hash(content_hash).is_err() || referenced.contains(content_hash) {
                continue;
            }
            fs::remove_file(entry.path())?;
            removed_any = true;
        }
        if removed_any {
            sync_dir(&dir)?;
        }
        Ok(())
    }
    fn ensure(&self) -> Result<(), JournalError> {
        if self.failed {
            Err(JournalError::RecoveryRequired)
        } else {
            Ok(())
        }
    }
    fn append(&mut self, event: Event) -> Result<(), JournalError> {
        let seq = self
            .projection
            .last_sequence
            .checked_add(1)
            .ok_or(JournalError::SequenceOverflow)?;
        let frame = Frame {
            sequence: seq,
            event,
        };
        let mut next = self.projection.clone();
        next.apply(&frame)?;
        let data = serde_json::to_vec(&frame)?;
        if data.len() > FRAME_LIMIT {
            return Err(JournalError::JournalFull);
        }
        if self
            .file
            .metadata()?
            .len()
            .saturating_add(data.len() as u64 + 4 + HASH_BYTES as u64)
            > JOURNAL_LIMIT
        {
            return Err(JournalError::JournalFull);
        }
        let mut raw = Vec::with_capacity(data.len() + 36);
        raw.extend_from_slice(&(data.len() as u32).to_le_bytes());
        raw.extend_from_slice(&data);
        raw.extend_from_slice(&hash(&data));
        if let Err(e) = self.file.write_all(&raw).and_then(|_| self.file.sync_all()) {
            self.failed = true;
            return Err(JournalError::Io(e));
        }
        self.projection = next;
        Ok(())
    }
}
fn request_fingerprint(s: &Submission) -> Result<String, JournalError> {
    let bytes = serde_json::to_vec(s)?;
    Ok(hex_digest(&bytes))
}
fn hex_digest(b: &[u8]) -> String {
    hash(b).iter().map(|v| format!("{v:02x}")).collect()
}
fn same_request(entry: &Entry, incoming: &Submission) -> Result<bool, JournalError> {
    if entry.request_fingerprint == request_fingerprint(incoming)? {
        return Ok(true);
    }
    // Gateway policy facts were added to already-supported request metadata.
    // Match an earlier admission's exact original request without rewriting it.
    let mut legacy = incoming.clone();
    if legacy
        .request_metadata
        .as_object_mut()
        .and_then(|metadata| metadata.remove("admission"))
        .is_some()
    {
        return Ok(entry.request_fingerprint == request_fingerprint(&legacy)?);
    }
    Ok(false)
}
fn validate_content_hash(value: &str) -> Result<(), JournalError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(JournalError::InvalidIdentifier("attachment content hash"));
    }
    Ok(())
}
fn valid_id(s: &str, kind: &'static str) -> Result<(), JournalError> {
    if s.is_empty() || s.len() > 1024 || s.bytes().any(|b| b == 0 || b.is_ascii_control()) {
        Err(JournalError::InvalidIdentifier(kind))
    } else {
        Ok(())
    }
}
fn hash(b: &[u8]) -> [u8; HASH_BYTES] {
    Sha256::digest(b).into()
}
fn owner_header(magic: &[u8; 8], id: RelOwnerId) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_BYTES as usize);
    header.extend_from_slice(magic);
    header.extend_from_slice(id.0.as_bytes());
    let checksum = hash(&header);
    header.extend_from_slice(&checksum);
    header
}
fn recover(f: &mut File, expected: RelOwnerId) -> Result<(Projection, u64), JournalError> {
    f.seek(SeekFrom::Start(0))?;
    let len = f.metadata()?.len();
    if len < HEADER_BYTES {
        return Err(JournalError::Corrupt("short header"));
    }
    let mut magic = [0; 8];
    f.read_exact(&mut magic)?;
    let mut owner = [0; 16];
    f.read_exact(&mut owner)?;
    if owner != *expected.0.as_bytes() {
        return Err(JournalError::Corrupt("owner UUID mismatch"));
    }
    let mut checksum = [0; HASH_BYTES];
    f.read_exact(&mut checksum)?;
    let mut header_bytes = Vec::with_capacity(24);
    header_bytes.extend_from_slice(&magic);
    header_bytes.extend_from_slice(&owner);
    if hash(&header_bytes) != checksum {
        return Err(JournalError::Corrupt("owner header checksum mismatch"));
    }
    let mut p = Projection::empty(expected);
    let mut pos = HEADER_BYTES;
    if &magic == SNAP_MAGIC {
        let mut b = [0; 8];
        f.read_exact(&mut b)?;
        let n = u64::from_le_bytes(b);
        if n > JOURNAL_LIMIT {
            return Err(JournalError::Corrupt("snapshot exceeds bound"));
        }
        let end = pos
            .checked_add(8)
            .and_then(|x| x.checked_add(n))
            .and_then(|x| x.checked_add(32))
            .ok_or(JournalError::Corrupt("snapshot length overflow"))?;
        if end > len {
            return Err(JournalError::Corrupt("incomplete snapshot"));
        }
        let mut data =
            vec![0; usize::try_from(n).map_err(|_| JournalError::Corrupt("snapshot too large"))?];
        f.read_exact(&mut data)?;
        let mut got = [0; 32];
        f.read_exact(&mut got)?;
        if hash(&data) != got {
            return Err(JournalError::Corrupt("snapshot checksum mismatch"));
        }
        p = Projection::from_snapshot(serde_json::from_slice(&data)?);
        if p.rel_id != expected {
            return Err(JournalError::Corrupt("snapshot owner mismatch"));
        }
        pos = end
    } else if &magic != FILE_MAGIC {
        return Err(JournalError::Corrupt("unknown header"));
    }
    let mut good = pos;
    while pos < len {
        f.seek(SeekFrom::Start(pos))?;
        let mut lb = [0; 4];
        if let Err(e) = f.read_exact(&mut lb) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            }
            return Err(e.into());
        }
        let n = u32::from_le_bytes(lb) as usize;
        if n == 0 || n > FRAME_LIMIT {
            return Err(JournalError::Corrupt("invalid complete frame length"));
        }
        let end = pos
            .checked_add(4 + n as u64 + 32)
            .ok_or(JournalError::Corrupt("frame overflow"))?;
        if end > len {
            break;
        }
        let mut data = vec![0; n];
        f.read_exact(&mut data)?;
        let mut got = [0; 32];
        f.read_exact(&mut got)?;
        if hash(&data) != got {
            return Err(JournalError::Corrupt("complete frame checksum mismatch"));
        }
        p.apply(&serde_json::from_slice::<Frame>(&data)?)?;
        pos = end;
        good = pos
    }
    Ok((p, good))
}
fn sync_dir(p: &Path) -> Result<(), JournalError> {
    #[cfg(unix)]
    {
        File::open(p)?.sync_all()?;
        Ok(())
    }
    #[cfg(windows)]
    {
        windows_sync_dir(p)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = p;
        Err(JournalError::UnsupportedDurability)
    }
}
#[cfg(unix)]
fn replace(a: &Path, b: &Path) -> Result<(), JournalError> {
    fs::rename(a, b)?;
    Ok(())
}
#[cfg(windows)]
fn replace(a: &Path, b: &Path) -> Result<(), JournalError> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
    }
    let a_path = canonicalized_child(a)?;
    let a: Vec<u16> = a_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let b_path = canonicalized_child(b)?;
    let b: Vec<u16> = b_path.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 1 | 8) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
#[cfg(windows)]
fn canonicalized_child(path: &Path) -> Result<PathBuf, JournalError> {
    let parent = path.parent().ok_or(JournalError::UnsupportedDurability)?;
    let parent = fs::canonicalize(parent)?;
    let name = path
        .file_name()
        .ok_or(JournalError::UnsupportedDurability)?;
    Ok(parent.join(name))
}
#[cfg(windows)]
fn windows_sync_dir(p: &Path) -> Result<(), JournalError> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            n: *const u16,
            a: u32,
            s: u32,
            sa: *mut core::ffi::c_void,
            c: u32,
            f: u32,
            t: *mut core::ffi::c_void,
        ) -> *mut core::ffi::c_void;
        fn FlushFileBuffers(h: *mut core::ffi::c_void) -> i32;
        fn CloseHandle(h: *mut core::ffi::c_void) -> i32;
    }
    let path = fs::canonicalize(p)?;
    let n: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let h = unsafe {
        CreateFileW(
            n.as_ptr(),
            0x40000000,
            7,
            null_mut(),
            3,
            0x02000000,
            null_mut(),
        )
    };
    if h as isize == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    let ok = unsafe { FlushFileBuffers(h) };
    let e = if ok == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe {
        CloseHandle(h);
    }
    if let Some(e) = e {
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "multiplexing_journal_tests.rs"]
mod tests;
