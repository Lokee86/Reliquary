//! Owner-bound operational queue groundwork with its own versioned framing.
//!
//! This journal is deliberately separate from REL semantic data. It records
//! accepted provisional submissions and the winner of cancel/finalize races.
//! Only the trusted Instance finalization path records Archive commitment, after
//! the mounted REL writer syncs the turn and branch. Host status recorders remain
//! private. Provider dispatch, recipient streaming, and reconnect transport are
//! not implemented; checkpoint reads are bounded local replay only, so H03 is
//! not complete.

use crate::OwnerId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

// Deliberately distinct from the reference RQMUXJ01: this reduced groundwork
// does not implement the reference's complete event/snapshot payload schema.
const JOURNAL_MAGIC: &[u8; 8] = b"RQGQ0001";
const HEADER_BYTES: u64 = 56;
const CHECKSUM_BYTES: usize = 32;
const FRAME_LIMIT: usize = 16 * 1024 * 1024;
const JOURNAL_LIMIT: u64 = 1024 * 1024 * 1024;
const MAX_CHECKPOINT_TEXT_BYTES: usize = 1024 * 1024;
const MAX_REPLAY_CHECKPOINTS: usize = 64;
const MAX_REPLAY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct SubmissionId(String);
impl SubmissionId {
    pub fn new(value: impl Into<String>) -> Result<Self, QueueError> {
        let value = value.into();
        validate_id(&value, "submission id")?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ConversationSubmissionId {
    pub conversation_id: String,
    pub submission_id: SubmissionId,
}
impl ConversationSubmissionId {
    pub fn new(
        conversation_id: impl Into<String>,
        submission_id: SubmissionId,
    ) -> Result<Self, QueueError> {
        let conversation_id = conversation_id.into();
        validate_id(&conversation_id, "conversation id")?;
        Ok(Self {
            conversation_id,
            submission_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueuedSubmission {
    pub id: ConversationSubmissionId,
    pub principal_id: String,
    pub content: String,
    pub payload_hash: [u8; 32],
}
impl QueuedSubmission {
    pub fn new(
        id: ConversationSubmissionId,
        principal_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, QueueError> {
        let principal_id = principal_id.into();
        validate_id(&principal_id, "principal id")?;
        let content = content.into();
        let payload_hash = Sha256::digest(content.as_bytes()).into();
        Ok(Self {
            id,
            principal_id,
            content,
            payload_hash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FinalizeCapture {
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub committed_timestamp_ns: i64,
    pub verified_principal_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReceipt {
    pub node_id: String,
    pub branch_id: String,
    pub parent_node_id: Option<String>,
    pub principal_id: String,
    pub committed_timestamp_ns: i64,
    pub payload_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueueState {
    Pending,
    Cancelled,
    Finalizing,
    Committed,
}

/// A host execution stage whose operation journal is attached to one submission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum RuntimeCapability {
    ConversationExecution,
    Insomnia,
    Dream,
    Embedding,
    ProjectEnvironment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostOperationStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostCancelOutcome {
    CancelledBeforeStart,
    RunningCancellationRequested,
    AlreadyTerminal(HostOperationStatus),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DurableHostOperationPhase {
    HandoffIntent,
    Accepted,
    Pending,
    Running,
    CancelIntent,
    RunningCancellationRequested,
    Held,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableHostOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub phase: DurableHostOperationPhase,
    pub transition_revision: u64,
    pub cancel_requested: bool,
    /// Last authoritative non-Unknown status, retained while phase is Held.
    pub last_known_phase: Option<DurableHostOperationPhase>,
}
impl DurableHostOperation {
    pub fn held(&self) -> bool {
        self.phase == DurableHostOperationPhase::Held
            || self.phase == DurableHostOperationPhase::HandoffIntent
    }
    pub fn requires_reconciliation(&self) -> bool {
        self.held()
            || matches!(
                self.phase,
                DurableHostOperationPhase::Accepted
                    | DurableHostOperationPhase::Pending
                    | DurableHostOperationPhase::Running
                    | DurableHostOperationPhase::CancelIntent
                    | DurableHostOperationPhase::RunningCancellationRequested
            )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HandoffReceipt {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub durably_accepted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmissionStatus {
    pub state: QueueState,
    pub admission_sequence: u64,
    pub transition_revision: u64,
    pub finalize_capture: Option<FinalizeCapture>,
    pub archive_receipt: Option<ArchiveReceipt>,
    pub host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
    pub generation: Option<DurableGeneration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DurableGenerationPhase {
    Pending,
    Running,
    CancellationRequested,
    Interrupted,
    Held,
    Unknown,
    Completed,
    Cancelled,
}
impl DurableGenerationPhase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Interrupted | Self::Completed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableGenerationCheckpoint {
    pub sequence: u64,
    /// Per-generation recipient-stream watermark, independent from the journal
    /// frame sequence and from checkpoint sequence.
    pub delivery_cursor: u64,
    pub utf8_bytes: u64,
    pub sha256: [u8; 32],
    /// The bounded full prefix is included in the synced queue event so replay
    /// can never expose content whose checkpoint metadata has not been synced.
    pub content_prefix: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableGeneration {
    pub operation_id: String,
    pub generation_id: String,
    pub branch_id: String,
    pub parent_node_id: String,
    pub started_timestamp_ns: i64,
    pub phase: DurableGenerationPhase,
    pub transition_revision: u64,
    pub checkpoint: Option<DurableGenerationCheckpoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationReplay {
    Checkpoints {
        latest_cursor: u64,
        items: Vec<DurableGenerationCheckpoint>,
    },
    ResyncRequired {
        latest_cursor: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnqueueOutcome {
    Durable(SubmissionStatus),
    Idempotent(SubmissionStatus),
}

#[derive(Debug)]
pub enum QueueError {
    Io(std::io::Error),
    Serialization(serde_json::Error),
    InvalidIdentifier(&'static str),
    OwnerMismatch,
    Corrupt(&'static str),
    DuplicatePayloadConflict,
    MissingSubmission,
    InvalidState,
    StaleRevision { expected: u64, actual: u64 },
    ArchiveIdentityConflict,
    HostOperationConflict,
    InvalidReplayCursor,
    InvalidReplayBound,
    SequenceOverflow,
    JournalFull,
    RecoveryRequired,
    UnsupportedDurability,
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "conversation queue I/O error: {error}"),
            Self::Serialization(error) => write!(f, "conversation queue encoding error: {error}"),
            Self::InvalidIdentifier(kind) => write!(f, "invalid {kind}"),
            Self::OwnerMismatch => {
                f.write_str("conversation queue belongs to a different REL owner")
            }
            Self::Corrupt(reason) => write!(f, "conversation queue corruption: {reason}"),
            Self::DuplicatePayloadConflict => {
                f.write_str("submission identity conflicts with stored payload")
            }
            Self::MissingSubmission => f.write_str("submission is not in the queue"),
            Self::InvalidState => {
                f.write_str("submission is not in a state that permits this transition")
            }
            Self::StaleRevision { expected, actual } => write!(
                f,
                "stale conversation revision: expected {expected}, actual {actual}"
            ),
            Self::ArchiveIdentityConflict => {
                f.write_str("Archive receipt does not match the captured submission")
            }
            Self::HostOperationConflict => {
                f.write_str("host operation identity or durable receipt conflicts")
            }
            Self::InvalidReplayCursor => {
                f.write_str("generation replay cursor is outside the durable checkpoint range")
            }
            Self::InvalidReplayBound => {
                f.write_str("generation replay bound is outside the supported limits")
            }
            Self::SequenceOverflow => f.write_str("conversation queue sequence overflow"),
            Self::JournalFull => f.write_str("conversation queue reached its size limit"),
            Self::RecoveryRequired => {
                f.write_str("conversation queue is fenced after a failed durable append")
            }
            Self::UnsupportedDurability => {
                f.write_str("required conversation queue durability is unavailable")
            }
        }
    }
}
impl std::error::Error for QueueError {}
impl From<std::io::Error> for QueueError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<serde_json::Error> for QueueError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Entry {
    submission: QueuedSubmission,
    state: QueueState,
    admission_sequence: u64,
    transition_revision: u64,
    finalize_capture: Option<FinalizeCapture>,
    archive_receipt: Option<ArchiveReceipt>,
    host_operations: BTreeMap<RuntimeCapability, DurableHostOperation>,
    generation: Option<DurableGeneration>,
    checkpoint_history: VecDeque<DurableGenerationCheckpoint>,
}
impl Entry {
    fn status(&self) -> SubmissionStatus {
        SubmissionStatus {
            state: self.state,
            admission_sequence: self.admission_sequence,
            transition_revision: self.transition_revision,
            finalize_capture: self.finalize_capture.clone(),
            archive_receipt: self.archive_receipt.clone(),
            host_operations: self.host_operations.clone(),
            generation: self.generation.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Projection {
    owner: [u8; 16],
    sequence: u64,
    conversation_revisions: BTreeMap<String, u64>,
    entries: BTreeMap<ConversationSubmissionId, Entry>,
}
impl Projection {
    fn empty(owner: OwnerId) -> Self {
        Self {
            owner: owner.as_bytes(),
            sequence: 0,
            conversation_revisions: BTreeMap::new(),
            entries: BTreeMap::new(),
        }
    }
    fn revision(&self, conversation_id: &str) -> u64 {
        self.conversation_revisions
            .get(conversation_id)
            .copied()
            .unwrap_or(0)
    }
    fn apply(&mut self, frame: &Frame) -> Result<(), QueueError> {
        if frame.sequence
            != self
                .sequence
                .checked_add(1)
                .ok_or(QueueError::SequenceOverflow)?
        {
            return Err(QueueError::Corrupt("sequence gap"));
        }
        match &frame.event {
            Event::Enqueued { submission } => {
                validate_id(&submission.id.conversation_id, "conversation id")?;
                validate_id(submission.id.submission_id.as_str(), "submission id")?;
                validate_id(&submission.principal_id, "principal id")?;
                let payload_hash: [u8; 32] = Sha256::digest(submission.content.as_bytes()).into();
                if payload_hash != submission.payload_hash {
                    return Err(QueueError::Corrupt("submission payload hash mismatch"));
                }
                if self.entries.contains_key(&submission.id) {
                    return Err(QueueError::Corrupt("duplicate enqueue"));
                }
                self.entries.insert(
                    submission.id.clone(),
                    Entry {
                        submission: submission.clone(),
                        state: QueueState::Pending,
                        admission_sequence: frame.sequence,
                        transition_revision: 1,
                        finalize_capture: None,
                        archive_receipt: None,
                        host_operations: BTreeMap::new(),
                        generation: None,
                        checkpoint_history: VecDeque::new(),
                    },
                );
            }
            Event::Cancelled {
                id,
                expected_revision,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(QueueError::Corrupt("stale cancel revision"));
                }
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing cancel target"))?;
                if entry.state != QueueState::Pending {
                    return Err(QueueError::Corrupt("cancel lost state"));
                }
                if !entry
                    .host_operations
                    .values()
                    .all(|op| op.phase == DurableHostOperationPhase::Cancelled)
                    || entry.generation.as_ref().is_some_and(|generation| {
                        generation.phase != DurableGenerationPhase::Cancelled
                    })
                {
                    return Err(QueueError::Corrupt("cancel before host cancellation"));
                }
                entry.state = QueueState::Cancelled;
                entry.transition_revision += 1;
            }
            Event::FinalizeWon {
                id,
                expected_revision,
                capture,
            } => {
                if self.revision(&id.conversation_id) != *expected_revision {
                    return Err(QueueError::Corrupt("stale finalize revision"));
                }
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing finalize target"))?;
                if entry.state != QueueState::Pending
                    || capture.verified_principal_id != entry.submission.principal_id
                    || !entry
                        .host_operations
                        .values()
                        .all(|op| op.phase == DurableHostOperationPhase::Completed)
                    || entry.generation.as_ref().is_some_and(|generation| {
                        generation.phase != DurableGenerationPhase::Completed
                    })
                {
                    return Err(QueueError::Corrupt("invalid finalize claim"));
                }
                entry.state = QueueState::Finalizing;
                entry.finalize_capture = Some(capture.clone());
                entry.transition_revision += 1;
            }
            Event::ArchiveCommitted { id, receipt } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing commit target"))?;
                validate_receipt(entry, receipt)?;
                if entry.state != QueueState::Finalizing {
                    return Err(QueueError::Corrupt("commit before finalize"));
                }
                entry.state = QueueState::Committed;
                entry.archive_receipt = Some(receipt.clone());
                entry.transition_revision += 1;
            }
            Event::HostOperationUpdated {
                id,
                capability,
                operation,
            } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing host operation target"))?;
                if entry.state != QueueState::Pending {
                    return Err(QueueError::Corrupt(
                        "host operation after native finalization",
                    ));
                }
                validate_host_operation_transition(
                    entry.host_operations.get(capability),
                    operation,
                )?;
                entry.host_operations.insert(*capability, operation.clone());
            }
            Event::GenerationUpdated { id, generation } => {
                let entry = self
                    .entries
                    .get_mut(id)
                    .ok_or(QueueError::Corrupt("missing generation target"))?;
                let new_checkpoint =
                    validate_generation_update(entry.generation.as_ref(), generation)?;
                if let Some(checkpoint) = new_checkpoint {
                    entry.checkpoint_history.push_back(checkpoint);
                    while entry.checkpoint_history.len() > MAX_REPLAY_CHECKPOINTS
                        || checkpoint_history_bytes(&entry.checkpoint_history) > MAX_REPLAY_BYTES
                    {
                        entry.checkpoint_history.pop_front();
                    }
                }
                entry.generation = Some(generation.clone());
            }
        }
        if !matches!(
            frame.event,
            Event::HostOperationUpdated { .. } | Event::GenerationUpdated { .. }
        ) {
            let conversation_id = frame.event.id().conversation_id.clone();
            *self
                .conversation_revisions
                .entry(conversation_id)
                .or_default() += 1;
        }
        self.sequence = frame.sequence;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Frame {
    sequence: u64,
    event: Event,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Event {
    Enqueued {
        submission: QueuedSubmission,
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
    HostOperationUpdated {
        id: ConversationSubmissionId,
        capability: RuntimeCapability,
        operation: DurableHostOperation,
    },
    GenerationUpdated {
        id: ConversationSubmissionId,
        generation: DurableGeneration,
    },
}
impl Event {
    fn id(&self) -> &ConversationSubmissionId {
        match self {
            Self::Enqueued { submission } => &submission.id,
            Self::Cancelled { id, .. }
            | Self::FinalizeWon { id, .. }
            | Self::ArchiveCommitted { id, .. }
            | Self::HostOperationUpdated { id, .. }
            | Self::GenerationUpdated { id, .. } => id,
        }
    }
}

/// Proof created only inside the trusted Archive publication integration.
/// No public constructor is provided; only the gateway's Archive publication
/// path can mint this after the REL writer has synced the turn and branch.
pub(crate) struct ArchiveCommitProof(ArchiveReceipt);
impl ArchiveCommitProof {
    pub(super) fn from_archive(receipt: ArchiveReceipt) -> Self {
        Self(receipt)
    }
}

/// One exclusive, REL-owner-qualified durable provisional conversation queue.
pub struct ConversationQueue {
    owner: OwnerId,
    path: PathBuf,
    file: File,
    projection: Projection,
    failed: bool,
    _writer_lock: File,
}
impl ConversationQueue {
    pub(crate) fn open(state_dir: impl AsRef<Path>, owner: OwnerId) -> Result<Self, QueueError> {
        let dir = state_dir.as_ref().join("conversation-queues");
        fs::create_dir_all(&dir)?;
        sync_dir(state_dir.as_ref())?;
        let path = dir.join(format!("{}.rqj", uuid_filename(owner.as_bytes())));
        let lock_path = path.with_extension("rqj.lock");
        let writer_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(lock_path)?;
        writer_lock.try_lock().map_err(|error| {
            QueueError::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                error.to_string(),
            ))
        })?;
        let existed = path.exists();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        if !existed {
            file.write_all(&owner_header(owner))?;
            file.sync_all()?;
            sync_dir(&dir)?;
        }
        let (projection, good_end) = recover(&mut file, owner)?;
        if file.metadata()?.len() > good_end {
            file.set_len(good_end)?;
            file.sync_all()?;
        }
        file.seek(SeekFrom::End(0))?;
        Ok(Self {
            owner,
            path,
            file,
            projection,
            failed: false,
            _writer_lock: writer_lock,
        })
    }
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn sequence(&self) -> u64 {
        self.projection.sequence
    }
    pub fn conversation_revision(&self, conversation_id: &str) -> u64 {
        self.projection.revision(conversation_id)
    }
    pub(crate) fn status(&self, id: &ConversationSubmissionId) -> Option<SubmissionStatus> {
        self.projection.entries.get(id).map(Entry::status)
    }
    pub(crate) fn host_operation(
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
    pub(crate) fn generation_checkpoint(
        &self,
        id: &ConversationSubmissionId,
    ) -> Option<DurableGenerationCheckpoint> {
        self.projection
            .entries
            .get(id)?
            .generation
            .as_ref()?
            .checkpoint
            .clone()
    }
    pub(crate) fn replay_generation_checkpoints(
        &self,
        id: &ConversationSubmissionId,
        after_cursor: u64,
        max_items: usize,
    ) -> Result<GenerationReplay, QueueError> {
        if max_items == 0 || max_items > MAX_REPLAY_CHECKPOINTS {
            return Err(QueueError::InvalidReplayBound);
        }
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        let latest_cursor = entry
            .generation
            .as_ref()
            .and_then(|generation| generation.checkpoint.as_ref())
            .map_or(0, |checkpoint| checkpoint.delivery_cursor);
        if after_cursor > latest_cursor {
            return Err(QueueError::InvalidReplayCursor);
        }
        let Some(first) = entry.checkpoint_history.front() else {
            return Ok(GenerationReplay::Checkpoints {
                latest_cursor,
                items: Vec::new(),
            });
        };
        if after_cursor.saturating_add(1) < first.delivery_cursor {
            return Ok(GenerationReplay::ResyncRequired { latest_cursor });
        }
        let items: Vec<_> = entry
            .checkpoint_history
            .iter()
            .filter(|checkpoint| checkpoint.delivery_cursor > after_cursor)
            .cloned()
            .collect();
        let bytes: usize = items.iter().map(|item| item.content_prefix.len()).sum();
        if items.len() > max_items || bytes > MAX_REPLAY_BYTES {
            return Ok(GenerationReplay::ResyncRequired { latest_cursor });
        }
        Ok(GenerationReplay::Checkpoints {
            latest_cursor,
            items,
        })
    }
    #[allow(dead_code)] // retained for the future authorized conversation reader
    pub(crate) fn submission(&self, id: &ConversationSubmissionId) -> Option<&QueuedSubmission> {
        self.projection
            .entries
            .get(id)
            .filter(|e| e.state == QueueState::Pending || e.state == QueueState::Finalizing)
            .map(|e| &e.submission)
    }
    pub(crate) fn pending_in_order(
        &self,
        conversation_id: &str,
    ) -> Vec<(ConversationSubmissionId, SubmissionStatus)> {
        let mut pending: Vec<_> = self
            .projection
            .entries
            .iter()
            .filter(|(id, entry)| {
                id.conversation_id == conversation_id
                    && entry.state == QueueState::Pending
                    && entry.host_operations.is_empty()
                    && entry.generation.is_none()
            })
            .map(|(id, entry)| (id.clone(), entry.status()))
            .collect();
        pending.sort_by_key(|(_, status)| status.admission_sequence);
        pending
    }
    pub(crate) fn enqueue(
        &mut self,
        submission: QueuedSubmission,
        expected_revision: u64,
    ) -> Result<EnqueueOutcome, QueueError> {
        self.ensure()?;
        validate_id(&submission.principal_id, "principal id")?;
        let content_hash: [u8; 32] = Sha256::digest(submission.content.as_bytes()).into();
        if content_hash != submission.payload_hash {
            return Err(QueueError::DuplicatePayloadConflict);
        }
        if let Some(entry) = self.projection.entries.get(&submission.id) {
            if entry.submission != submission {
                return Err(QueueError::DuplicatePayloadConflict);
            }
            return Ok(EnqueueOutcome::Idempotent(entry.status()));
        }
        self.check_revision(&submission.id.conversation_id, expected_revision)?;
        let id = submission.id.clone();
        self.append(Event::Enqueued { submission })?;
        Ok(EnqueueOutcome::Durable(
            self.status(&id).expect("new entry is projected"),
        ))
    }
    pub(crate) fn cancel(
        &mut self,
        id: &ConversationSubmissionId,
        expected_revision: u64,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Cancelled {
            return Ok(entry.status());
        }
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if !entry
            .host_operations
            .values()
            .all(|op| op.phase == DurableHostOperationPhase::Cancelled)
            || entry
                .generation
                .as_ref()
                .is_some_and(|generation| generation.phase != DurableGenerationPhase::Cancelled)
        {
            return Err(QueueError::InvalidState);
        }
        self.check_revision(&id.conversation_id, expected_revision)?;
        self.append(Event::Cancelled {
            id: id.clone(),
            expected_revision,
        })?;
        Ok(self.status(id).expect("cancelled entry is projected"))
    }
    pub(crate) fn claim_finalize(
        &mut self,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Finalizing {
            if entry.finalize_capture.as_ref() == Some(&capture) {
                return Ok(entry.status());
            }
            return Err(QueueError::ArchiveIdentityConflict);
        }
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if !entry
            .host_operations
            .values()
            .all(|op| op.phase == DurableHostOperationPhase::Completed)
            || entry
                .generation
                .as_ref()
                .is_some_and(|generation| generation.phase != DurableGenerationPhase::Completed)
        {
            return Err(QueueError::InvalidState);
        }
        validate_id(&capture.branch_id, "branch id")?;
        if let Some(parent) = &capture.parent_node_id {
            validate_id(parent, "parent node id")?;
        }
        if capture.verified_principal_id != entry.submission.principal_id {
            return Err(QueueError::ArchiveIdentityConflict);
        }
        self.check_revision(&id.conversation_id, expected_revision)?;
        self.append(Event::FinalizeWon {
            id: id.clone(),
            expected_revision,
            capture,
        })?;
        Ok(self.status(id).expect("finalizing entry is projected"))
    }
    /// Record an Archive receipt only from the private publication seam.
    #[allow(dead_code)] // no writable Archive owner seam exists in this rebuild yet
    pub(crate) fn record_archive_commit(
        &mut self,
        id: &ConversationSubmissionId,
        proof: ArchiveCommitProof,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state == QueueState::Committed {
            if entry.archive_receipt.as_ref() == Some(&proof.0) {
                return Ok(entry.status());
            }
            return Err(QueueError::ArchiveIdentityConflict);
        }
        if entry.state != QueueState::Finalizing {
            return Err(QueueError::InvalidState);
        }
        validate_receipt(entry, &proof.0)?;
        self.append(Event::ArchiveCommitted {
            id: id.clone(),
            receipt: proof.0,
        })?;
        Ok(self.status(id).expect("committed entry is projected"))
    }

    /// Persist one stable operation intent. `true` means a new intent was
    /// appended; `false` means one already exists and MUST NOT be redispatched.
    #[allow(dead_code)] // the host dispatcher is deliberately not implemented here
    pub(crate) fn begin_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
    ) -> Result<bool, QueueError> {
        validate_id(&stable_id, "host operation id")?;
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if let Some(previous) = entry.host_operations.get(&capability) {
            if previous.stable_id != stable_id || previous.payload_digest != payload_digest {
                return Err(QueueError::HostOperationConflict);
            }
            return Ok(false);
        }
        self.set_host_operation_phase(
            id,
            capability,
            stable_id,
            payload_digest,
            DurableHostOperationPhase::HandoffIntent,
        )?;
        Ok(true)
    }

    #[allow(dead_code)] // only a real durable host adapter may provide this receipt
    pub(crate) fn record_host_handoff(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        receipt: &HandoffReceipt,
    ) -> Result<SubmissionStatus, QueueError> {
        if !receipt.durably_accepted {
            return Err(QueueError::HostOperationConflict);
        }
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        if current.stable_id != receipt.stable_id
            || current.payload_digest != receipt.payload_digest
        {
            return Err(QueueError::HostOperationConflict);
        }
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
            return Ok(self.status(id).expect("operation belongs to queue entry"));
        }
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            DurableHostOperationPhase::Accepted,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // wired by startup/reconnect reconciliation in the host runtime
    pub(crate) fn record_host_status(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        status: HostOperationStatus,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
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
            HostOperationStatus::Unknown => DurableHostOperationPhase::Held,
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // no external host cancellation is dispatched by this crate
    pub(crate) fn begin_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            DurableHostOperationPhase::CancelIntent,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // only a conforming host adapter may provide this outcome
    pub(crate) fn record_host_cancel(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        outcome: HostCancelOutcome,
    ) -> Result<SubmissionStatus, QueueError> {
        let current = self
            .host_operation(id, capability)
            .ok_or(QueueError::MissingSubmission)?;
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
                DurableHostOperationPhase::Held
            }
        };
        self.set_host_operation_phase(
            id,
            capability,
            current.stable_id,
            current.payload_digest,
            phase,
        )?;
        Ok(self.status(id).expect("operation belongs to queue entry"))
    }

    #[allow(dead_code)] // trusted generation driver only; no public provider dispatch exists
    pub(crate) fn begin_generation(
        &mut self,
        id: &ConversationSubmissionId,
        operation_id: String,
        generation_id: String,
        branch_id: String,
        parent_node_id: String,
        started_timestamp_ns: i64,
    ) -> Result<SubmissionStatus, QueueError> {
        for (value, kind) in [
            (&operation_id, "generation operation id"),
            (&generation_id, "generation id"),
            (&branch_id, "branch id"),
            (&parent_node_id, "parent node id"),
        ] {
            validate_id(value, kind)?;
        }
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        if let Some(previous) = &entry.generation {
            if previous.operation_id == operation_id
                && previous.generation_id == generation_id
                && previous.branch_id == branch_id
                && previous.parent_node_id == parent_node_id
                && previous.started_timestamp_ns == started_timestamp_ns
            {
                return Ok(entry.status());
            }
            return Err(QueueError::HostOperationConflict);
        }
        let generation = DurableGeneration {
            operation_id,
            generation_id,
            branch_id,
            parent_node_id,
            started_timestamp_ns,
            phase: DurableGenerationPhase::Running,
            transition_revision: 1,
            checkpoint: None,
        };
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }

    #[allow(dead_code)] // trusted generation driver only; content becomes visible after sync
    pub(crate) fn record_generation_checkpoint(
        &mut self,
        id: &ConversationSubmissionId,
        sequence: u64,
        content_prefix: String,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        if content_prefix.len() > MAX_CHECKPOINT_TEXT_BYTES {
            return Err(QueueError::JournalFull);
        }
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        let current = entry.generation.as_ref().ok_or(QueueError::InvalidState)?;
        if current.phase != DurableGenerationPhase::Running {
            return Err(QueueError::InvalidState);
        }
        let bytes = u64::try_from(content_prefix.len()).map_err(|_| QueueError::JournalFull)?;
        let digest: [u8; 32] = Sha256::digest(content_prefix.as_bytes()).into();
        if let Some(previous) = &current.checkpoint {
            if sequence == previous.sequence {
                if previous.utf8_bytes == bytes
                    && previous.sha256 == digest
                    && previous.content_prefix == content_prefix
                {
                    return Ok(entry.status());
                }
                return Err(QueueError::DuplicatePayloadConflict);
            }
            if sequence
                != previous
                    .sequence
                    .checked_add(1)
                    .ok_or(QueueError::SequenceOverflow)?
            {
                return Err(QueueError::InvalidReplayCursor);
            }
            if !content_prefix.starts_with(&previous.content_prefix) {
                return Err(QueueError::DuplicatePayloadConflict);
            }
        } else if sequence != 1 {
            return Err(QueueError::InvalidReplayCursor);
        }
        let mut next = current.clone();
        next.transition_revision = next
            .transition_revision
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        let delivery_cursor = current
            .checkpoint
            .as_ref()
            .map_or(Some(1), |checkpoint| {
                checkpoint.delivery_cursor.checked_add(1)
            })
            .ok_or(QueueError::SequenceOverflow)?;
        next.checkpoint = Some(DurableGenerationCheckpoint {
            sequence,
            delivery_cursor,
            utf8_bytes: bytes,
            sha256: digest,
            content_prefix,
        });
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation: next,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }

    #[allow(dead_code)] // lifecycle reconciliation remains an internal trusted seam
    pub(crate) fn set_generation_phase(
        &mut self,
        id: &ConversationSubmissionId,
        phase: DurableGenerationPhase,
    ) -> Result<SubmissionStatus, QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        let mut generation = entry.generation.clone().ok_or(QueueError::InvalidState)?;
        if generation.phase == phase {
            return Ok(entry.status());
        }
        if !generation_phase_transition_allowed(generation.phase, phase) {
            return Err(QueueError::InvalidState);
        }
        generation.phase = phase;
        generation.transition_revision = generation
            .transition_revision
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        self.append(Event::GenerationUpdated {
            id: id.clone(),
            generation,
        })?;
        Ok(self.status(id).expect("generation belongs to queue entry"))
    }

    #[allow(dead_code)] // transition primitive for the future host runtime seam
    fn set_host_operation_phase(
        &mut self,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        stable_id: String,
        payload_digest: [u8; 32],
        phase: DurableHostOperationPhase,
    ) -> Result<(), QueueError> {
        self.ensure()?;
        let entry = self
            .projection
            .entries
            .get(id)
            .ok_or(QueueError::MissingSubmission)?;
        if entry.state != QueueState::Pending {
            return Err(QueueError::InvalidState);
        }
        let previous = entry.host_operations.get(&capability);
        let (revision, cancel_requested, last_known_phase) = match previous {
            Some(old) => {
                if old.stable_id != stable_id || old.payload_digest != payload_digest {
                    return Err(QueueError::HostOperationConflict);
                }
                if old.phase == phase {
                    return Ok(());
                }
                if !host_phase_transition_allowed(&old.phase, &phase) {
                    if old.phase != DurableHostOperationPhase::Held
                        && matches!(
                            phase,
                            DurableHostOperationPhase::Pending | DurableHostOperationPhase::Running
                        )
                        && host_phase_transition_allowed(&phase, &old.phase)
                    {
                        return Ok(());
                    }
                    return Err(QueueError::InvalidState);
                }
                let revision = old
                    .transition_revision
                    .checked_add(1)
                    .ok_or(QueueError::SequenceOverflow)?;
                let sticky_cancel = old.cancel_requested
                    || matches!(
                        phase,
                        DurableHostOperationPhase::CancelIntent
                            | DurableHostOperationPhase::RunningCancellationRequested
                    );
                let last_known = if phase == DurableHostOperationPhase::Held {
                    Some(
                        old.last_known_phase
                            .clone()
                            .unwrap_or_else(|| old.phase.clone()),
                    )
                } else {
                    Some(phase.clone())
                };
                (revision, sticky_cancel, last_known)
            }
            None => {
                if phase != DurableHostOperationPhase::HandoffIntent {
                    return Err(QueueError::InvalidState);
                }
                (1, false, None)
            }
        };
        self.append(Event::HostOperationUpdated {
            id: id.clone(),
            capability,
            operation: DurableHostOperation {
                stable_id,
                payload_digest,
                phase,
                transition_revision: revision,
                cancel_requested,
                last_known_phase,
            },
        })
    }
    pub(crate) fn submitter_principal(&self, id: &ConversationSubmissionId) -> Option<&str> {
        self.projection
            .entries
            .get(id)
            .map(|entry| entry.submission.principal_id.as_str())
    }
    fn check_revision(&self, conversation_id: &str, expected: u64) -> Result<(), QueueError> {
        let actual = self.conversation_revision(conversation_id);
        if actual == expected {
            Ok(())
        } else {
            Err(QueueError::StaleRevision { expected, actual })
        }
    }
    fn ensure(&self) -> Result<(), QueueError> {
        if self.failed {
            Err(QueueError::RecoveryRequired)
        } else {
            Ok(())
        }
    }
    fn append(&mut self, event: Event) -> Result<(), QueueError> {
        let sequence = self
            .projection
            .sequence
            .checked_add(1)
            .ok_or(QueueError::SequenceOverflow)?;
        let frame = Frame { sequence, event };
        let mut next = self.projection.clone();
        next.apply(&frame)?;
        let bytes = serde_json::to_vec(&frame)?;
        if bytes.is_empty() || bytes.len() > FRAME_LIMIT {
            return Err(QueueError::JournalFull);
        }
        if self
            .file
            .metadata()?
            .len()
            .saturating_add(bytes.len() as u64 + 4 + CHECKSUM_BYTES as u64)
            > JOURNAL_LIMIT
        {
            return Err(QueueError::JournalFull);
        }
        let mut raw = Vec::with_capacity(bytes.len() + 4 + CHECKSUM_BYTES);
        raw.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        raw.extend_from_slice(&bytes);
        raw.extend_from_slice(&sha256(&bytes));
        if let Err(error) = self.file.write_all(&raw).and_then(|_| self.file.sync_all()) {
            self.failed = true;
            return Err(QueueError::Io(error));
        }
        self.projection = next;
        Ok(())
    }
}

fn validate_receipt(entry: &Entry, receipt: &ArchiveReceipt) -> Result<(), QueueError> {
    let capture = entry
        .finalize_capture
        .as_ref()
        .ok_or(QueueError::Corrupt("missing finalize capture"))?;
    if receipt.node_id != entry.submission.id.submission_id.as_str()
        || receipt.branch_id != capture.branch_id
        || receipt.parent_node_id != capture.parent_node_id
        || receipt.principal_id != capture.verified_principal_id
        || receipt.committed_timestamp_ns != capture.committed_timestamp_ns
        || receipt.payload_hash != entry.submission.payload_hash
    {
        return Err(QueueError::ArchiveIdentityConflict);
    }
    Ok(())
}

fn validate_host_operation_transition(
    previous: Option<&DurableHostOperation>,
    operation: &DurableHostOperation,
) -> Result<(), QueueError> {
    validate_id(&operation.stable_id, "host operation id")?;
    match previous {
        None => {
            if operation.transition_revision != 1
                || operation.phase != DurableHostOperationPhase::HandoffIntent
                || operation.cancel_requested
                || operation.last_known_phase.is_some()
            {
                return Err(QueueError::Corrupt("invalid initial host operation state"));
            }
        }
        Some(old) => {
            if old.stable_id != operation.stable_id
                || old.payload_digest != operation.payload_digest
                || Some(operation.transition_revision) != old.transition_revision.checked_add(1)
                || (!operation.cancel_requested && old.cancel_requested)
                || (matches!(
                    operation.phase,
                    DurableHostOperationPhase::CancelIntent
                        | DurableHostOperationPhase::RunningCancellationRequested
                ) && !operation.cancel_requested)
                || !host_phase_transition_allowed(&old.phase, &operation.phase)
            {
                return Err(QueueError::Corrupt("invalid host operation transition"));
            }
            let expected_last_known = if operation.phase == DurableHostOperationPhase::Held {
                Some(
                    old.last_known_phase
                        .clone()
                        .unwrap_or_else(|| old.phase.clone()),
                )
            } else {
                Some(operation.phase.clone())
            };
            if operation.last_known_phase != expected_last_known {
                return Err(QueueError::Corrupt(
                    "invalid last-known host operation phase",
                ));
            }
        }
    }
    Ok(())
}

fn host_phase_transition_allowed(
    old: &DurableHostOperationPhase,
    new: &DurableHostOperationPhase,
) -> bool {
    use DurableHostOperationPhase::*;
    match old {
        HandoffIntent => matches!(
            new,
            Accepted | Pending | Running | CancelIntent | Cancelled | Completed | Held
        ),
        Accepted => matches!(
            new,
            Pending | Running | CancelIntent | Cancelled | Completed | Held
        ),
        Pending => matches!(new, Running | CancelIntent | Cancelled | Completed | Held),
        Running => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Completed | Held
        ),
        CancelIntent => matches!(
            new,
            Pending | Running | RunningCancellationRequested | Cancelled | Completed | Held
        ),
        RunningCancellationRequested => matches!(
            new,
            CancelIntent | RunningCancellationRequested | Cancelled | Completed | Held
        ),
        Held => matches!(
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

fn validate_generation_update(
    previous: Option<&DurableGeneration>,
    next: &DurableGeneration,
) -> Result<Option<DurableGenerationCheckpoint>, QueueError> {
    for (value, kind) in [
        (&next.operation_id, "generation operation id"),
        (&next.generation_id, "generation id"),
        (&next.branch_id, "branch id"),
        (&next.parent_node_id, "parent node id"),
    ] {
        validate_id(value, kind)?;
    }
    match previous {
        None => {
            if next.transition_revision != 1
                || !matches!(
                    next.phase,
                    DurableGenerationPhase::Pending | DurableGenerationPhase::Running
                )
                || next.checkpoint.is_some()
            {
                return Err(QueueError::Corrupt("invalid initial generation state"));
            }
            Ok(None)
        }
        Some(old) => {
            if old.phase.terminal()
                || old.operation_id != next.operation_id
                || old.generation_id != next.generation_id
                || old.branch_id != next.branch_id
                || old.parent_node_id != next.parent_node_id
                || old.started_timestamp_ns != next.started_timestamp_ns
                || Some(next.transition_revision) != old.transition_revision.checked_add(1)
                || (old.phase != next.phase
                    && !generation_phase_transition_allowed(old.phase, next.phase))
            {
                return Err(QueueError::Corrupt(
                    "generation identity or phase transition",
                ));
            }
            match (&old.checkpoint, &next.checkpoint) {
                (None, None) => Ok(None),
                (None, Some(checkpoint))
                    if checkpoint.sequence == 1 && checkpoint.delivery_cursor == 1 =>
                {
                    validate_checkpoint(checkpoint, None)?;
                    Ok(Some(checkpoint.clone()))
                }
                (Some(previous), Some(checkpoint)) if checkpoint == previous => Ok(None),
                (Some(previous), Some(checkpoint))
                    if Some(checkpoint.sequence) == previous.sequence.checked_add(1)
                        && Some(checkpoint.delivery_cursor)
                            == previous.delivery_cursor.checked_add(1)
                        && checkpoint.utf8_bytes >= previous.utf8_bytes
                        && checkpoint
                            .content_prefix
                            .starts_with(&previous.content_prefix) =>
                {
                    validate_checkpoint(checkpoint, Some(previous))?;
                    Ok(Some(checkpoint.clone()))
                }
                _ => Err(QueueError::Corrupt(
                    "invalid generation checkpoint transition",
                )),
            }
        }
    }
}

fn validate_checkpoint(
    checkpoint: &DurableGenerationCheckpoint,
    previous: Option<&DurableGenerationCheckpoint>,
) -> Result<(), QueueError> {
    let bytes = checkpoint.content_prefix.len();
    let digest: [u8; 32] = Sha256::digest(checkpoint.content_prefix.as_bytes()).into();
    if checkpoint.sequence == 0
        || checkpoint.delivery_cursor == 0
        || bytes > MAX_CHECKPOINT_TEXT_BYTES
        || checkpoint.utf8_bytes != bytes as u64
        || digest != checkpoint.sha256
        || previous.is_some_and(|old| !checkpoint.content_prefix.starts_with(&old.content_prefix))
    {
        return Err(QueueError::Corrupt(
            "checkpoint content does not match durable identity",
        ));
    }
    Ok(())
}

fn checkpoint_history_bytes(history: &VecDeque<DurableGenerationCheckpoint>) -> usize {
    history
        .iter()
        .map(|checkpoint| checkpoint.content_prefix.len())
        .sum()
}

fn generation_phase_transition_allowed(
    old: DurableGenerationPhase,
    new: DurableGenerationPhase,
) -> bool {
    use DurableGenerationPhase::*;
    match old {
        Pending => matches!(
            new,
            Running | CancellationRequested | Interrupted | Held | Unknown | Completed | Cancelled
        ),
        Running => matches!(
            new,
            CancellationRequested | Interrupted | Held | Unknown | Completed
        ),
        CancellationRequested => {
            matches!(new, Interrupted | Held | Unknown | Completed | Cancelled)
        }
        Held | Unknown => matches!(
            new,
            Pending | Running | CancellationRequested | Interrupted | Completed | Cancelled
        ),
        Interrupted | Completed | Cancelled => false,
    }
}

fn validate_id(value: &str, kind: &'static str) -> Result<(), QueueError> {
    if value.is_empty()
        || value.len() > 1024
        || value.bytes().any(|b| b == 0 || b.is_ascii_control())
    {
        Err(QueueError::InvalidIdentifier(kind))
    } else {
        Ok(())
    }
}
fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn uuid_filename(bytes: [u8; 16]) -> String {
    let raw = hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8],
        &raw[8..12],
        &raw[12..16],
        &raw[16..20],
        &raw[20..32]
    )
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn owner_header(owner: OwnerId) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_BYTES as usize);
    header.extend_from_slice(JOURNAL_MAGIC);
    header.extend_from_slice(&owner.as_bytes());
    let checksum = sha256(&header);
    header.extend_from_slice(&checksum);
    header
}
fn recover(file: &mut File, expected: OwnerId) -> Result<(Projection, u64), QueueError> {
    file.seek(SeekFrom::Start(0))?;
    let length = file.metadata()?.len();
    if length < HEADER_BYTES {
        return Err(QueueError::Corrupt("short owner header"));
    }
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    if &magic != JOURNAL_MAGIC {
        return Err(QueueError::Corrupt("unknown journal header"));
    }
    let mut owner = [0; 16];
    file.read_exact(&mut owner)?;
    if owner != expected.as_bytes() {
        return Err(QueueError::OwnerMismatch);
    }
    let mut checksum = [0; CHECKSUM_BYTES];
    file.read_exact(&mut checksum)?;
    let mut header = Vec::with_capacity(24);
    header.extend_from_slice(&magic);
    header.extend_from_slice(&owner);
    if sha256(&header) != checksum {
        return Err(QueueError::Corrupt("owner header checksum"));
    }
    let mut projection = Projection::empty(expected);
    let mut position = HEADER_BYTES;
    let mut good_end = position;
    while position < length {
        file.seek(SeekFrom::Start(position))?;
        let mut len_bytes = [0; 4];
        if let Err(error) = file.read_exact(&mut len_bytes) {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            }
            return Err(error.into());
        }
        let frame_len = u32::from_le_bytes(len_bytes) as usize;
        if frame_len == 0 || frame_len > FRAME_LIMIT {
            return Err(QueueError::Corrupt("invalid complete frame length"));
        }
        let end = position
            .checked_add(4 + frame_len as u64 + CHECKSUM_BYTES as u64)
            .ok_or(QueueError::Corrupt("frame length overflow"))?;
        if end > length {
            break;
        }
        let mut data = vec![0; frame_len];
        file.read_exact(&mut data)?;
        let mut got = [0; CHECKSUM_BYTES];
        file.read_exact(&mut got)?;
        if sha256(&data) != got {
            return Err(QueueError::Corrupt("complete frame checksum"));
        }
        projection.apply(&serde_json::from_slice::<Frame>(&data)?)?;
        position = end;
        good_end = end;
    }
    Ok((projection, good_end))
}
fn sync_dir(path: &Path) -> Result<(), QueueError> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateFileW(
                name: *const u16,
                access: u32,
                share: u32,
                security: *mut core::ffi::c_void,
                creation: u32,
                flags: u32,
                template: *mut core::ffi::c_void,
            ) -> *mut core::ffi::c_void;
            fn FlushFileBuffers(handle: *mut core::ffi::c_void) -> i32;
            fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
        }
        let path = fs::canonicalize(path)?;
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                0x40000000,
                7,
                null_mut(),
                3,
                0x02000000,
                null_mut(),
            )
        };
        if handle as isize == -1 {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = unsafe { FlushFileBuffers(handle) };
        let error = (result == 0).then(std::io::Error::last_os_error);
        unsafe {
            CloseHandle(handle);
        }
        if let Some(error) = error {
            return Err(error.into());
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(QueueError::UnsupportedDurability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("queue-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn submission(text: &str) -> QueuedSubmission {
        QueuedSubmission::new(
            ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
                .unwrap(),
            "alice",
            text,
        )
        .unwrap()
    }

    #[test]
    fn queue_is_owner_bound_durable_and_replays_ordered_pending_work() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([17; 16]);
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            queue.enqueue(submission("hello"), 0).unwrap();
            assert_eq!(queue.pending_in_order("conversation").len(), 1);
        }
        let wrong_owner = OwnerId::from_bytes([18; 16]);
        let original = dir
            .join("conversation-queues")
            .join(format!("{}.rqj", uuid_filename(owner.as_bytes())));
        let mismatched = dir
            .join("conversation-queues")
            .join(format!("{}.rqj", uuid_filename(wrong_owner.as_bytes())));
        fs::copy(original, mismatched).unwrap();
        assert!(matches!(
            ConversationQueue::open(&dir, wrong_owner),
            Err(QueueError::OwnerMismatch)
        ));
        let queue = ConversationQueue::open(&dir, owner).unwrap();
        assert_eq!(queue.sequence(), 1);
        assert_eq!(queue.pending_in_order("conversation").len(), 1);
        drop(queue);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancel_and_finalize_have_one_durable_winner_and_finalize_needs_receipt_proof() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([19; 16]);
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        let submission = submission("hello");
        let id = submission.id.clone();
        queue.enqueue(submission.clone(), 0).unwrap();
        let capture = FinalizeCapture {
            branch_id: "main".into(),
            parent_node_id: None,
            committed_timestamp_ns: 7,
            verified_principal_id: "alice".into(),
        };
        let finalized = queue.claim_finalize(&id, 1, capture.clone()).unwrap();
        assert_eq!(finalized.state, QueueState::Finalizing);
        assert!(matches!(
            queue.cancel(&id, 1),
            Err(QueueError::InvalidState)
        ));
        assert_eq!(queue.claim_finalize(&id, 1, capture).unwrap(), finalized);
        assert!(queue.status(&id).unwrap().archive_receipt.is_none());

        let cancelled_submission = QueuedSubmission::new(
            ConversationSubmissionId::new(
                "conversation",
                SubmissionId::new("cancelled-submission").unwrap(),
            )
            .unwrap(),
            "alice",
            "cancel me",
        )
        .unwrap();
        let cancelled_id = cancelled_submission.id.clone();
        queue.enqueue(cancelled_submission, 2).unwrap();
        assert_eq!(
            queue.cancel(&cancelled_id, 3).unwrap().state,
            QueueState::Cancelled
        );
        assert!(matches!(
            queue.claim_finalize(
                &cancelled_id,
                4,
                FinalizeCapture {
                    branch_id: "main".into(),
                    parent_node_id: None,
                    committed_timestamp_ns: 8,
                    verified_principal_id: "alice".into(),
                }
            ),
            Err(QueueError::InvalidState)
        ));

        let wrong = ArchiveCommitProof(ArchiveReceipt {
            node_id: "other-node".into(),
            branch_id: "main".into(),
            parent_node_id: None,
            principal_id: "alice".into(),
            committed_timestamp_ns: 7,
            payload_hash: submission.payload_hash,
        });
        assert!(matches!(
            queue.record_archive_commit(&id, wrong),
            Err(QueueError::ArchiveIdentityConflict)
        ));
        assert_eq!(queue.status(&id).unwrap().state, QueueState::Finalizing);
        drop(queue);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn duplicate_enqueue_requires_identical_payload_and_revision() {
        let dir = temp_dir();
        let mut queue = ConversationQueue::open(&dir, OwnerId::from_bytes([20; 16])).unwrap();
        queue.enqueue(submission("first"), 0).unwrap();
        assert!(matches!(
            queue.enqueue(submission("different"), 1),
            Err(QueueError::DuplicatePayloadConflict)
        ));
        assert!(matches!(
            queue.enqueue(
                QueuedSubmission::new(
                    ConversationSubmissionId::new(
                        "conversation",
                        SubmissionId::new("other").unwrap()
                    )
                    .unwrap(),
                    "alice",
                    "second"
                )
                .unwrap(),
                0
            ),
            Err(QueueError::StaleRevision { .. })
        ));
        drop(queue);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovery_discards_only_an_incomplete_final_frame() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([21; 16]);
        let path;
        let valid_len;
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            queue.enqueue(submission("durable"), 0).unwrap();
            path = queue.path().to_path_buf();
            valid_len = fs::metadata(&path).unwrap().len();
        }
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            file.write_all(&[12, 0]).unwrap();
            file.sync_all().unwrap();
        }
        {
            let queue = ConversationQueue::open(&dir, owner).unwrap();
            assert_eq!(queue.sequence(), 1);
            assert_eq!(fs::metadata(&path).unwrap().len(), valid_len);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovery_rejects_a_corrupt_complete_frame() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([22; 16]);
        let path;
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            queue.enqueue(submission("durable"), 0).unwrap();
            path = queue.path().to_path_buf();
        }
        {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            let len = file.metadata().unwrap().len();
            file.seek(SeekFrom::Start(len - 1)).unwrap();
            let mut byte = [0];
            file.read_exact(&mut byte).unwrap();
            byte[0] ^= 0xff;
            file.seek(SeekFrom::Start(len - 1)).unwrap();
            file.write_all(&byte).unwrap();
            file.sync_all().unwrap();
        }
        assert!(matches!(
            ConversationQueue::open(&dir, owner),
            Err(QueueError::Corrupt("complete frame checksum"))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_host_status_is_durable_held_and_never_recreates_dispatch_intent() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([24; 16]);
        let stable_id = "op-24".to_owned();
        let digest = [9; 32];
        let submission = submission("host work");
        let id = submission.id.clone();
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            queue.enqueue(submission, 0).unwrap();
            assert!(
                queue
                    .begin_host_handoff(
                        &id,
                        RuntimeCapability::ConversationExecution,
                        stable_id.clone(),
                        digest
                    )
                    .unwrap()
            );
        }
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            // Existing intent means status reconciliation, never redispatch.
            assert!(
                !queue
                    .begin_host_handoff(
                        &id,
                        RuntimeCapability::ConversationExecution,
                        stable_id.clone(),
                        digest
                    )
                    .unwrap()
            );
            let held = queue
                .record_host_status(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    HostOperationStatus::Unknown,
                )
                .unwrap();
            let operation = held
                .host_operations
                .get(&RuntimeCapability::ConversationExecution)
                .unwrap();
            assert_eq!(operation.phase, DurableHostOperationPhase::Held);
            assert_eq!(
                operation.last_known_phase,
                Some(DurableHostOperationPhase::HandoffIntent)
            );
            assert!(operation.held());
            assert!(operation.requires_reconciliation());
            drop(queue);
        }
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            let operation = queue
                .host_operation(&id, RuntimeCapability::ConversationExecution)
                .unwrap();
            assert_eq!(operation.phase, DurableHostOperationPhase::Held);
            assert_eq!(
                operation.last_known_phase,
                Some(DurableHostOperationPhase::HandoffIntent)
            );
            assert!(operation.held());
            assert!(operation.requires_reconciliation());
            assert!(
                !queue
                    .begin_host_handoff(
                        &id,
                        RuntimeCapability::ConversationExecution,
                        stable_id.clone(),
                        digest
                    )
                    .unwrap()
            );
            assert!(queue.pending_in_order("conversation").is_empty());
            assert!(matches!(
                queue.claim_finalize(
                    &id,
                    1,
                    FinalizeCapture {
                        branch_id: "main".into(),
                        parent_node_id: None,
                        committed_timestamp_ns: 9,
                        verified_principal_id: "alice".into(),
                    }
                ),
                Err(QueueError::InvalidState)
            ));

            let pending = queue
                .record_host_status(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    HostOperationStatus::Pending,
                )
                .unwrap();
            let operation = pending
                .host_operations
                .get(&RuntimeCapability::ConversationExecution)
                .unwrap();
            assert_eq!(operation.phase, DurableHostOperationPhase::Pending);
            assert!(!operation.held());
            // Even a known in-flight operation is not scheduled for a second handoff.
            assert!(queue.pending_in_order("conversation").is_empty());
            let completed = queue
                .record_host_status(
                    &id,
                    RuntimeCapability::ConversationExecution,
                    HostOperationStatus::Completed,
                )
                .unwrap();
            assert_eq!(
                completed.host_operations[&RuntimeCapability::ConversationExecution].phase,
                DurableHostOperationPhase::Completed
            );
            assert!(
                queue
                    .claim_finalize(
                        &id,
                        1,
                        FinalizeCapture {
                            branch_id: "main".into(),
                            parent_node_id: None,
                            committed_timestamp_ns: 9,
                            verified_principal_id: "alice".into(),
                        }
                    )
                    .is_ok()
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_host_cancel_keeps_cancel_intent_and_cancel_requires_terminal_proof() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([25; 16]);
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        let submission = submission("cancel host work");
        let id = submission.id.clone();
        queue.enqueue(submission, 0).unwrap();
        queue
            .begin_host_handoff(
                &id,
                RuntimeCapability::ConversationExecution,
                "op-25".into(),
                [8; 32],
            )
            .unwrap();
        queue
            .begin_host_cancel(&id, RuntimeCapability::ConversationExecution)
            .unwrap();
        let held = queue
            .record_host_cancel(
                &id,
                RuntimeCapability::ConversationExecution,
                HostCancelOutcome::Unknown,
            )
            .unwrap();
        let operation = held
            .host_operations
            .get(&RuntimeCapability::ConversationExecution)
            .unwrap();
        assert_eq!(operation.phase, DurableHostOperationPhase::Held);
        assert!(operation.cancel_requested);
        assert_eq!(
            operation.last_known_phase,
            Some(DurableHostOperationPhase::CancelIntent)
        );
        assert!(matches!(
            queue.cancel(&id, 1),
            Err(QueueError::InvalidState)
        ));
        let cancelled = queue
            .record_host_cancel(
                &id,
                RuntimeCapability::ConversationExecution,
                HostCancelOutcome::CancelledBeforeStart,
            )
            .unwrap();
        assert_eq!(
            cancelled.host_operations[&RuntimeCapability::ConversationExecution].phase,
            DurableHostOperationPhase::Cancelled
        );
        assert_eq!(queue.cancel(&id, 1).unwrap().state, QueueState::Cancelled);
        drop(queue);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn generation_checkpoints_are_synced_recovered_and_replayed_with_bounds() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([26; 16]);
        let submission = submission("generate");
        let id = submission.id.clone();
        {
            let mut queue = ConversationQueue::open(&dir, owner).unwrap();
            queue.enqueue(submission, 0).unwrap();
            queue
                .begin_generation(
                    &id,
                    "op-26".into(),
                    "gen-26".into(),
                    "main".into(),
                    "parent".into(),
                    10,
                )
                .unwrap();
            for sequence in 1u64..=70 {
                let prefix = "x".repeat(sequence as usize);
                queue
                    .record_generation_checkpoint(&id, sequence, prefix)
                    .unwrap();
            }
            let before_idempotent = queue.sequence();
            queue
                .record_generation_checkpoint(&id, 70, "x".repeat(70))
                .unwrap();
            assert_eq!(queue.sequence(), before_idempotent);
            assert!(matches!(
                queue.replay_generation_checkpoints(&id, 0, 10).unwrap(),
                GenerationReplay::ResyncRequired { latest_cursor: 70 }
            ));
        }
        {
            let queue = ConversationQueue::open(&dir, owner).unwrap();
            let latest = queue.generation_checkpoint(&id).unwrap();
            assert_eq!(latest.sequence, 70);
            assert_eq!(latest.content_prefix, "x".repeat(70));
            assert_eq!(latest.utf8_bytes, 70);
            assert_eq!(
                queue.replay_generation_checkpoints(&id, 69, 1).unwrap(),
                GenerationReplay::Checkpoints {
                    latest_cursor: 70,
                    items: vec![latest]
                }
            );
            assert!(matches!(
                queue.replay_generation_checkpoints(&id, 70, 0),
                Err(QueueError::InvalidReplayBound)
            ));
            assert!(matches!(
                queue.replay_generation_checkpoints(&id, 71, 1),
                Err(QueueError::InvalidReplayCursor)
            ));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_checkpoint_append_never_becomes_visible_before_sync() {
        let dir = temp_dir();
        let owner = OwnerId::from_bytes([27; 16]);
        let submission = submission("generate");
        let id = submission.id.clone();
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        queue.enqueue(submission, 0).unwrap();
        queue
            .begin_generation(
                &id,
                "op-27".into(),
                "gen-27".into(),
                "main".into(),
                "parent".into(),
                11,
            )
            .unwrap();
        let path = queue.path().to_path_buf();
        queue.file = OpenOptions::new().read(true).open(&path).unwrap();
        assert!(matches!(
            queue.record_generation_checkpoint(&id, 1, "uncommitted output".into()),
            Err(QueueError::Io(_))
        ));
        assert!(queue.generation_checkpoint(&id).is_none());
        drop(queue);
        let queue = ConversationQueue::open(&dir, owner).unwrap();
        assert!(queue.generation_checkpoint(&id).is_none());
        drop(queue);
        fs::remove_dir_all(dir).unwrap();
    }
}
